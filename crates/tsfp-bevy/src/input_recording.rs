// SPDX-License-Identifier: GPL-3.0-or-later
//! T2348 native input capture at the Rust input-router boundary.

use crate::shared::lock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tsfp_input::player::PlayerInput;
use tsfp_input::recording::{
    DEVICE_ROWS_VERSION, Mark, Payload, Pointer, Reader, Record, Tick, Ui,
    UiEvent as RecordedUiEvent, Writer,
};
use tsfp_input::shim::{SdlReport, ShimEvent, diff, report_from_player};

struct Capture {
    writer: Writer<BufWriter<File>>,
    started: Instant,
}

/// T3820: the shell side of the device intake: clocks, `D` rows, live keys during a replay, the lockstep.
struct IntakeHost;

impl tsfp_input_intake::Host for IntakeHost {
    fn clock(&self) -> (u64, u64) {
        (crate::wi::simulation_tick(), crate::wi::host_poll_index())
    }
    fn write(&self, sim_tick: u64, host_poll: u64, payload: String) {
        if let Some(capture) = lock(&CAPTURE).as_mut() {
            let wall_ns = elapsed_ns(capture.started);
            let row = Payload {
                sim_tick,
                host_poll,
                wall_ns,
                payload,
            };
            if let Err(error) = capture.writer.device(&row) {
                eprintln!("tsfp-bevy: input recording device row failed: {error}");
            }
        }
    }
    fn replay_poll(&self) -> u64 {
        replay_poll()
    }
    fn live_key(&self, key: &bevy::input::keyboard::KeyboardInput) {
        lock(&crate::shared::SHARED.wi).feed(crate::events::raw_key(key)); // the host's Alt+F4 close chord
    }
    fn progress(&self) {
        publish_replay_progress();
    }
}

/// A replay of a format v5 or older recording: semantic `U` rows stand in for native-page input (T3820).
pub fn semantic_replay() -> bool {
    replaying() && !tsfp_input_intake::replaying()
}

static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);
/// T2487: guest polls past the recorded end after which undelivered UI rows stop holding the replay open.
const UI_ROWS_END_GRACE_POLLS: u64 = 600;
static REPLAY_ENABLED: AtomicBool = AtomicBool::new(false);

/// T3393: `TSFP_REPLAY_CLOCK=tick` keys replay delivery on the simulation tick instead of the
/// live host poll count. The host poll counts every window pump, including the presenter's idle
/// pumps, so its rate per tick follows the wall clock (MEASURED story-level1-20261018: the
/// recording 2.92 polls per tick, a paced replay 3.0, fast-forward 1.0). In tick mode the replay
/// poll is the recording's own host poll at the current tick, so recorded rows reach the same
/// simulation tick in every pacing mode and the recorded tick timing is reproduced.
pub fn tick_clock() -> bool {
    static TICK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TICK.get_or_init(|| tick_clock_switch(std::env::var("TSFP_REPLAY_CLOCK").ok().as_deref()))
}

pub fn tick_clock_switch(value: Option<&str>) -> bool {
    value == Some("tick")
}

/// Recorded (sim tick, host poll) pairs, poll made non-decreasing, sorted by tick.
static TICK_POLLS: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());

fn recorded_poll_at(table: &[(u64, u64)], tick: u64) -> u64 {
    match table.partition_point(|&(recorded, _)| recorded <= tick) {
        0 => 0,
        // Past the last recorded tick the poll keeps moving (one per tick), so the route end
        // (a poll past the last recorded one) is reached.
        index if index == table.len() => {
            let (last_tick, last_poll) = table[index - 1];
            last_poll + (tick - last_tick)
        }
        index => table[index - 1].1,
    }
}

/// The poll the replay schedule runs on: the live host poll, or (tick clock) the recorded poll
/// of the current simulation tick.
pub fn replay_poll() -> u64 {
    if !tick_clock() {
        return crate::wi::host_poll_index();
    }
    recorded_poll_at(&lock(&TICK_POLLS), crate::wi::simulation_tick())
}

struct Playback {
    ticks: VecDeque<Tick>,
    /// T3592: recorded physical pad (`G` rows) by sim tick, delivered to the guest on ticks whose `T` row is `-`
    /// (a native page owned input when recording) once no Rust page owns input during replay.
    native_pad: HashMap<u64, Option<PlayerInput>>,
    ui: VecDeque<Ui>,
    ui_ticks: HashSet<(u32, u64)>,
    /// T3392: recorded mouse pointer rows (format v5), delivered by host poll at the guest frame boundary.
    pointer: VecDeque<Pointer>,
    /// T3392: the recording holds at least one `P` row (false for v1 to v4 and for v5 files without a mouse).
    pointer_recorded: bool,
    /// T3392: host polls of every recorded A press edge of player 0 (immutable), for the legacy pointer shim.
    accept_polls: Vec<u64>,
    /// T3395: host polls of every recorded Start press edge of player 0, for read-only page diagnostics.
    start_polls: Vec<u64>,
    sent: SdlReport,
    end_tick: u64,
    end_poll: u64,
    first_event_tick: u64,
    timeline_anchor: Option<(u64, u64)>,
    hold_after_end: bool,
    handover_after_end: bool,
    hold_reported: bool,
    /// Exclusive guest poll count observed when the final recorded row is delivered.
    guest_exclusive_at_final_row: Option<u64>,
    /// T2487: UI rows still undelivered when the route ended were reported and abandoned.
    ui_abandoned_reported: bool,
    /// T2487: the route-end marker poll of an abandoned route (last row poll + 1, as for a normal end).
    end_marker_poll: Option<u64>,
    ticks_replayed: u64,
    /// T3607: recorded F12 marks `(index, mark)` not yet reached, reported once with the replay's
    /// own guest port-0 poll (the unit of `--snapshot-at-poll`, not the recorded host poll).
    marks: VecDeque<(usize, Mark)>,
    last_tick_inputs: Option<(u64, [Option<PlayerInput>; 4])>,
    button_press_poll: [Option<u64>; 15],
    poll_latch_wait_reported: u16,
}

impl Playback {
    fn rebase_to(&mut self, live_tick: u64, recorded_tick: u64) {
        self.timeline_anchor = Some((live_tick, recorded_tick));
    }

    /// T2487: drop the rows still queued past the route end and report them once. The route-end
    /// marker then names the recorded end (end poll + 1, as a normal end), so rust_trace's route
    /// alignment does not depend on the grace window or on when the stale rows would have drained.
    fn abandon_rows_after_end(&mut self) {
        if !self.ui_abandoned_reported {
            eprintln!(
                "tsfp-bevy: native input replay: {} tick row(s) and {} UI row(s) undelivered {} polls after the recorded end poll {}; abandoning them",
                self.ticks.len(),
                self.ui.len(),
                UI_ROWS_END_GRACE_POLLS,
                self.end_poll
            );
            // T3393: name every undelivered row without exposing text payloads.
            for event in &self.ui {
                eprintln!(
                    "tsfp-bevy: undelivered UI row page {:#x} tick {} poll {} context {} player {} kind {}",
                    event.page,
                    event.sim_tick,
                    event.host_poll,
                    event.context,
                    event.player,
                    ui_event_kind(&event.event),
                );
            }
            self.ui_abandoned_reported = true;
            self.end_marker_poll = Some(self.end_poll.saturating_add(1));
        }
        self.ticks.clear();
    }

    /// T3607: print `tools.gate` the guest port-0 poll at which the replay reaches each recorded
    /// F12 mark. The recorded `host_poll` of a mark counts live host polls (about two per tick), the
    /// replay's snapshot and trace polls count guest port-0 polls, so the two cannot be compared.
    fn report_marks_reached(&mut self, sim_tick: u64, guest_pad_polls: u64) {
        while self
            .marks
            .front()
            .is_some_and(|(_, mark)| mark.sim_tick <= sim_tick)
        {
            let (index, mark) = self.marks.pop_front().expect("front checked above");
            eprintln!(
                "tsfp-bevy: input replay mark {index} label={} sim_tick={} recorded_poll={} guest_pad_poll={guest_pad_polls}",
                mark.label.replace(char::is_whitespace, "_"),
                mark.sim_tick,
                mark.host_poll,
            );
        }
    }

    fn recorded_tick(&mut self, live_tick: u64) -> u64 {
        if self.timeline_anchor.is_none() && live_tick >= self.first_event_tick {
            self.timeline_anchor = Some((live_tick, self.first_event_tick));
        }
        let recorded_tick =
            self.timeline_anchor
                .map_or(live_tick, |(live_anchor, recorded_anchor)| {
                    if live_tick >= live_anchor {
                        recorded_anchor.saturating_add(live_tick - live_anchor)
                    } else {
                        recorded_anchor.saturating_sub(live_anchor - live_tick)
                    }
                });
        recorded_tick
    }

    fn step(&mut self, live_tick: u64, _elapsed_ns: u64) -> Result<ReplayStep, String> {
        self.step_with_poll(live_tick, 0, self.ticks_replayed)
    }

    fn step_with_poll(
        &mut self,
        live_tick: u64,
        _elapsed_ns: u64,
        guest_pad_polls: u64,
    ) -> Result<ReplayStep, String> {
        self.step_with_delivery(live_tick, 0, guest_pad_polls, true)
    }

    fn step_with_delivery(
        &mut self,
        live_tick: u64,
        _elapsed_ns: u64,
        guest_pad_polls: u64,
        guest_delivery: bool,
    ) -> Result<ReplayStep, String> {
        self.step_scheduled(
            live_tick,
            None,
            guest_pad_polls,
            guest_delivery,
            crate::ui_live::replay_active_page(),
            crate::ui_overlay::top_live_page().map(|page| page.builder),
        )
    }

    fn step_at_host_poll(
        &mut self,
        live_tick: u64,
        live_poll: u64,
        guest_pad_polls: u64,
        guest_delivery: bool,
    ) -> Result<ReplayStep, String> {
        self.step_at_host_poll_with_owners(
            live_tick,
            live_poll,
            guest_pad_polls,
            guest_delivery,
            crate::ui_live::replay_active_page(),
            crate::ui_overlay::top_live_page().map(|page| page.builder),
        )
    }

    fn step_at_host_poll_with_owners(
        &mut self,
        live_tick: u64,
        live_poll: u64,
        guest_pad_polls: u64,
        guest_delivery: bool,
        active_rust_page: Option<u32>,
        active_guest_page: Option<u32>,
    ) -> Result<ReplayStep, String> {
        self.step_scheduled(
            live_tick,
            Some(live_poll),
            guest_pad_polls,
            guest_delivery,
            active_rust_page,
            active_guest_page,
        )
    }

    fn step_scheduled(
        &mut self,
        live_tick: u64,
        live_poll: Option<u64>,
        guest_pad_polls: u64,
        guest_delivery: bool,
        active_rust_page: Option<u32>,
        active_guest_page: Option<u32>,
    ) -> Result<ReplayStep, String> {
        // T2487: tick rows still queued past the recorded end plus the grace window are stale
        // (fast-forward ran r5's last neutral rows to host poll 15499 against a recorded 7215).
        if !self.ticks.is_empty()
            && live_poll
                .is_some_and(|poll| poll > self.end_poll.saturating_add(UI_ROWS_END_GRACE_POLLS))
        {
            self.abandon_rows_after_end();
        }
        let mut sim_tick = self.recorded_tick(live_tick);
        let Some((next_tick, next_poll)) = self
            .ticks
            .front()
            .map(|tick| (tick.sim_tick, tick.host_poll))
        else {
            // UI rows are consumed by their owning page in `ui_events`. A
            // page transition can make one temporarily ineligible, but it
            // must not turn the route into a missed-tick error or stop the
            // gameplay clock while that page becomes active.
            if !self.ui.is_empty() || crate::ui_live::replay_page_commands_pending() {
                // T2487: a UI row whose page never became active must not keep the replay running
                // forever (r5 ran to poll 35184 against a recorded end at 7215). Past the recorded end
                // plus a grace window the remaining rows are reported and abandoned.
                let abandon = live_poll.is_some_and(|poll| {
                    poll > self.end_poll.saturating_add(UI_ROWS_END_GRACE_POLLS)
                });
                if !abandon {
                    return Ok(ReplayStep::Wait);
                }
                self.abandon_rows_after_end();
            }
            if live_poll.is_some_and(|poll| poll > self.end_poll)
                || (live_poll.is_none() && sim_tick > self.end_tick)
            {
                if !self.hold_reported {
                    tsfp_input_intake::route_boundary::arm(
                        self.end_line_poll(),
                        sim_tick,
                        self.hold_after_end,
                        self.guest_exclusive_at_final_row,
                    );
                    self.hold_reported = true;
                }
                if self.handover_after_end {
                    return Ok(ReplayStep::Handover);
                }
                if !self.hold_after_end {
                    return Ok(ReplayStep::End);
                }
                let target = SdlReport {
                    axes: [Some(0); 6],
                    ..SdlReport::default()
                };
                let mut events = Vec::new();
                let target =
                    self.keep_unobserved_guest_presses(target, guest_pad_polls, guest_delivery);
                let previous_buttons = self.sent.buttons;
                diff(&mut self.sent, &target, &mut events);
                for index in 0..15 {
                    let mask = 1u16 << index;
                    if previous_buttons & mask != 0 && self.sent.buttons & mask == 0 {
                        self.button_press_poll[index] = None;
                        self.poll_latch_wait_reported &= !mask;
                    }
                }
                return Ok(ReplayStep::Tick([None; 4], events));
            }
            return Ok(ReplayStep::Wait);
        };
        if let Some(live_poll) = live_poll {
            if next_poll > live_poll {
                return Ok(ReplayStep::Wait);
            }
        } else if next_tick < sim_tick {
            self.rebase_to(live_tick, next_tick);
            sim_tick = next_tick;
        } else if next_tick > sim_tick {
            return Ok(if sim_tick > self.end_tick {
                ReplayStep::End
            } else {
                ReplayStep::Wait
            });
        }
        if live_poll.is_none() && next_tick < sim_tick {
            return Err(format!(
                "replay missed sim tick {} before current tick {sim_tick}",
                next_tick
            ));
        }
        let tick = self.ticks.front().expect("front checked above");
        let gated = tsfp_input_intake::decisions::gate_control(tick.inputs); // T3873: the owner's control lock
        let mut input = gated[0];
        if input.is_none() && active_rust_page.is_none() {
            input = self.native_pad.get(&tick.sim_tick).copied().flatten();
        }
        let mut target = input.as_ref().map(report_from_player).unwrap_or(SdlReport {
            axes: [Some(0); 6],
            ..SdlReport::default()
        });
        if guest_delivery {
            // New recordings store native UI actions separately from the pad snapshot. A row's
            // recorded builder can lag a transition, so hold it until some current page owns
            // input; never inject it into an empty page stack.
            // The recording page is a capture-time hint. It can be stale across
            // a UI transition, so the current Rust page gets first ownership;
            // otherwise route one due row to the current guest page. Only pop
            // the queue head: scanning ahead can combine Back/Scroll presses
            // from different recorded polls and changes their route order.
            if let Some((row, button)) = take_guest_owned_ui_event(
                &mut self.ui,
                live_poll,
                active_rust_page,
                active_guest_page,
            ) {
                target.buttons |= 1u16 << button;
                eprintln!(
                    "replay-ui: delivered row poll {} device UI context {} player {} recorded page {:#x} kind {} event {:?} to guest owner {:#x} at poll {}",
                    row.host_poll,
                    row.context,
                    row.player,
                    row.page,
                    ui_event_kind(&row.event),
                    row.event,
                    active_guest_page.unwrap_or_default(),
                    live_poll.unwrap_or_default(),
                );
            }
        }
        let target = self.keep_unobserved_guest_presses(target, guest_pad_polls, guest_delivery);
        let tick = self.ticks.pop_front().expect("front checked above");
        if self.ticks.is_empty() && self.ui.is_empty() && tick.host_poll == self.end_poll {
            self.guest_exclusive_at_final_row = Some(guest_pad_polls);
        }
        self.ticks_replayed += 1;
        self.report_marks_reached(tick.sim_tick, guest_pad_polls);
        self.last_tick_inputs = Some((tick.sim_tick, tick.inputs));
        if self.hold_after_end && self.ticks_replayed % 120 == 0 {
            eprintln!(
                "tsfp-bevy: native input replay progress: {} of recorded ticks consumed (current recorded tick {})",
                self.ticks_replayed, tick.sim_tick
            );
        }
        let previous_buttons = self.sent.buttons;
        let mut events = Vec::new();
        diff(&mut self.sent, &target, &mut events);
        for index in 0..15 {
            let mask = 1u16 << index;
            if guest_delivery && previous_buttons & mask == 0 && self.sent.buttons & mask != 0 {
                self.button_press_poll[index] = Some(guest_pad_polls);
                self.poll_latch_wait_reported &= !mask;
            } else if previous_buttons & mask != 0 && self.sent.buttons & mask == 0 {
                self.button_press_poll[index] = None;
                self.poll_latch_wait_reported &= !mask;
            }
        }
        Ok(ReplayStep::Tick(gated, events))
    }

    fn keep_unobserved_guest_presses(
        &mut self,
        mut target: SdlReport,
        guest_pad_polls: u64,
        guest_delivery: bool,
    ) -> SdlReport {
        for index in 0..15 {
            let mask = 1u16 << index;
            let Some(press_poll) = self.button_press_poll[index] else {
                continue;
            };
            if guest_delivery && guest_pad_polls > press_poll {
                self.button_press_poll[index] = None;
                self.poll_latch_wait_reported &= !mask;
            } else if guest_delivery && target.buttons & mask == 0 {
                // Preserve a guest-bound press until XInput has observed it, but keep consuming
                // ordered recording rows. Blocking the stream here can deadlock transitions
                // whose guest code does not poll input while loading.
                target.buttons |= mask;
                if self.poll_latch_wait_reported & mask == 0 {
                    eprintln!(
                        "tsfp-bevy: native input replay keeping button {index} held until a guest pad poll observes it; continuing recorded rows"
                    );
                    self.poll_latch_wait_reported |= mask;
                }
            }
        }
        target
    }

    fn player_input(&mut self, live_tick: u64, _elapsed_ns: u64) -> Option<PlayerInput> {
        let recorded_tick = self.recorded_tick(live_tick);
        let inputs = self
            .ticks
            .front()
            .filter(|tick| tick.sim_tick == recorded_tick)
            .map(|tick| tick.inputs)
            .or_else(|| {
                self.last_tick_inputs
                    .as_ref()
                    .filter(|(tick, _)| *tick == recorded_tick)
                    .map(|(_, inputs)| *inputs)
            })?;
        inputs[0]
    }

    fn ui_events(
        &mut self,
        live_poll: u64,
        elapsed_ns: u64,
        active_page: Option<u32>,
    ) -> Result<Vec<Ui>, String> {
        self.ui_events_for_pages(live_poll, elapsed_ns, active_page, None)
    }

    fn ui_events_for_pages(
        &mut self,
        live_poll: u64,
        _elapsed_ns: u64,
        active_page: Option<u32>,
        active_text_page: Option<u32>,
    ) -> Result<Vec<Ui>, String> {
        // UI rows are scheduled on the recorded host-frame poll counter. Wall
        // timestamps describe the original session but must not decide when a
        // replayed action reaches the game.
        let Some(page) = active_page.filter(|page| native_ui_page_has_handler(*page)) else {
            return Ok(Vec::new());
        };
        let Some(first) = self.ui.front() else {
            return Ok(Vec::new());
        };
        if first.host_poll > live_poll || !ui_event_is_ready(first, page, active_text_page) {
            return Ok(Vec::new());
        }
        let next_poll = first.host_poll;
        let mut events = Vec::new();
        while self.ui.front().is_some_and(|event| {
            event.host_poll == next_poll
                && event.host_poll <= live_poll
                && ui_event_is_ready(event, page, active_text_page)
        }) {
            let mut event = self.ui.pop_front().expect("front event was checked above");
            if !matches!(
                &event.event,
                RecordedUiEvent::TextInput(_)
                    | RecordedUiEvent::TextCommit { .. }
                    | RecordedUiEvent::TextCancel(_)
            ) {
                // The recorded builder can lag a page transition by a host
                // poll. Dispatch the semantic action to the owner observed now.
                if event.page != page && std::env::var_os("TSFP_REPLAY_INPUT_LOG").is_some() {
                    eprintln!(
                        "replay-ui: retargeted row poll {} from recorded page {:#x} to Rust owner {page:#x} ({})",
                        event.host_poll,
                        event.page,
                        ui_event_kind(&event.event),
                    );
                }
                event.page = page;
            }
            events.push(event);
        }
        Ok(events)
    }
}

/// T3391: drop main-menu model rows that echo a native page's input. Recordings made before
/// the capture gate (`ui_live`: the main model records only while the main menu is live) hold
/// the main model's copy of every pad press made on another Rust page, in the same host poll
/// as that page's own row. Replayed, the stale copy waits until the main menu is next live and
/// then activates its row there (the Map Maker route's profile `A:0` opened Story instead of
/// Map Maker). Returns the number of rows dropped.
fn drop_main_menu_echoes(ui: &mut VecDeque<Ui>) -> usize {
    let main = tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32;
    let is_main = |page: u32| page == 0 || page == main;
    let other_page_polls: HashSet<u64> = ui
        .iter()
        .filter(|event| !is_main(event.page))
        .map(|event| event.host_poll)
        .collect();
    let before = ui.len();
    ui.retain(|event| !(is_main(event.page) && other_page_polls.contains(&event.host_poll)));
    before - ui.len()
}

fn ui_event_is_ready(event: &Ui, active_page: u32, active_text_page: Option<u32>) -> bool {
    // T3772: the profile page answers every player on its own port; other pages are player 1's.
    let player_ok = event.player == 0 || event.page == tsfp_ui_input::profile_page::BUILDER;
    if event.context != 0 || !player_ok || !native_ui_page_has_handler(active_page) {
        return false;
    }
    match &event.event {
        RecordedUiEvent::TextInput(_)
        | RecordedUiEvent::TextCommit { .. }
        | RecordedUiEvent::TextCancel(_) => {
            ui_page_matches(event.page, active_page) && active_text_page == Some(active_page)
        }
        RecordedUiEvent::Hover(_) | RecordedUiEvent::HoverLeave => false,
        RecordedUiEvent::Scroll(list, delta) => {
            *delta != 0 || tsfp_ui_input::page_intents::recorded_action_list(*list)
        }
        RecordedUiEvent::Focus(_)
        | RecordedUiEvent::Activate(_)
        | RecordedUiEvent::Back
        | RecordedUiEvent::Start => true,
    }
}

fn ui_event_kind(event: &RecordedUiEvent) -> &'static str {
    match event {
        RecordedUiEvent::Focus(_) => "Focus",
        RecordedUiEvent::Activate(_) => "Activate",
        RecordedUiEvent::Back => "Back",
        RecordedUiEvent::Start => "Start",
        RecordedUiEvent::Scroll(_, _) => "Scroll",
        RecordedUiEvent::Hover(_) => "Hover",
        RecordedUiEvent::HoverLeave => "HoverLeave",
        RecordedUiEvent::TextInput(_) => "TextInput",
        RecordedUiEvent::TextCommit { .. } => "TextCommit",
        RecordedUiEvent::TextCancel(_) => "TextCancel",
    }
}

/// SDL button that reproduces a semantic UI action when the guest, rather than a Rust page,
/// owns the controller poll. The recorded list IDs 0 and 1 match the native UI's vertical and
/// horizontal step axes. UI-only focus and text events have no faithful pad equivalent.
fn guest_ui_button(event: &RecordedUiEvent) -> Option<u8> {
    match event {
        // T3592: Start is the SDL Start button (6, XInput 0x10), not A. Replayed as A, the
        // owner's cutscene skip presses never left the Story cutscenes.
        RecordedUiEvent::Start => Some(6),
        RecordedUiEvent::Activate(_) => Some(0),
        RecordedUiEvent::Back => Some(1),
        RecordedUiEvent::Scroll(1, delta) if *delta < 0 => Some(13),
        RecordedUiEvent::Scroll(1, delta) if *delta > 0 => Some(14),
        RecordedUiEvent::Scroll(u32::MAX, _) => Some(14),
        RecordedUiEvent::Scroll(0xFFFF_FFFE, _) => Some(13),
        RecordedUiEvent::Scroll(l, d) => tsfp_ui_input::keyboard::list_scroll_button(*l, *d),
        _ => None,
    }
}

/// Registered pages, the main menu and (T3787) the `NATIVE_INPUT_PAGES` such as Arcade setup, as live input routes them.
pub(crate) fn native_ui_page_has_handler(page: u32) -> bool {
    page == 0 || crate::ui_live::popup_parents().contains(&page)
}

fn native_ui_page_owns_poll(recorded_page: u32, active_page: Option<u32>) -> bool {
    native_ui_page_has_handler(recorded_page)
        && active_page.is_some_and(|active_page| ui_page_matches(recorded_page, active_page))
}

fn guest_ui_button_for_page(
    _recorded_page: u32,
    event: &RecordedUiEvent,
    active_guest_page: Option<u32>,
) -> Option<u8> {
    active_guest_page?;
    guest_ui_button(event)
}

fn take_guest_owned_ui_event(
    ui: &mut VecDeque<Ui>,
    live_poll: Option<u64>,
    active_rust_page: Option<u32>,
    active_guest_page: Option<u32>,
) -> Option<(Ui, u8)> {
    let row = ui.front()?;
    if !live_poll.is_some_and(|poll| row.host_poll <= poll)
        || row.context != 0
        || row.player != 0
        || active_rust_page.is_some_and(native_ui_page_has_handler)
    {
        return None;
    }
    let button = guest_ui_button_for_page(row.page, &row.event, active_guest_page)?;
    Some((ui.pop_front()?, button))
}

fn ui_page_matches(recorded_page: u32, active_page: u32) -> bool {
    recorded_page == active_page
        || (recorded_page == 0 && active_page == tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32)
}

fn ui_page_has_event_at(
    ui_ticks: &HashSet<(u32, u64)>,
    active_page: Option<u32>,
    tick: u64,
) -> bool {
    active_page.is_some_and(|active| {
        ui_ticks.iter().any(|(recorded_page, recorded_tick)| {
            *recorded_tick == tick && ui_page_matches(*recorded_page, active)
        })
    })
}

pub enum ReplayStep {
    Wait,
    Tick([Option<PlayerInput>; 4], Vec<ShimEvent>),
    End,
    Handover,
}

fn check_unlock_policy(recorded: bool, requested: bool) -> Result<(), String> {
    if recorded == requested {
        Ok(())
    } else {
        Err(format!(
            "recording unlock-all policy is {recorded}, current launch is {requested}"
        ))
    }
}

static PLAYBACK: Mutex<Option<Playback>> = Mutex::new(None);

pub fn start(path: &Path, unlock_all: bool) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let file = File::create(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    let build = Some(env!("TSFP_BUILD_GIT_SHA"));
    let mut writer = Writer::stamped(BufWriter::new(file), unlock_all, build)
        .map_err(|error| error.to_string())?;
    writer
        .mark(&Mark {
            sim_tick: 0,
            host_poll: 0,
            wall_ns: 0,
            label: "record-start".into(),
        })
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    *lock(&CAPTURE) = Some(Capture { writer, started });
    ENABLED.store(true, Ordering::Release);
    tsfp_input_intake::set_host(Box::new(IntakeHost));
    tsfp_input_intake::start_recording();
    eprintln!(
        "tsfp-bevy: native input recording to {} (F12 adds a mark)",
        path.display()
    );
    Ok(())
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub fn start_replay(path: &Path, unlock_all: bool, handover_after_end: bool) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut reader = Reader::new(BufReader::new(file)).map_err(|error| error.to_string())?;
    check_unlock_policy(reader.unlock_all(), unlock_all)?;
    let mut ticks = VecDeque::new();
    let mut native_pad = HashMap::new();
    let mut ui = VecDeque::new();
    let mut pointer = VecDeque::new();
    let mut marks = VecDeque::new();
    let mut devices = Vec::new();
    while let Some(record) = reader.next().map_err(|error| error.to_string())? {
        match record {
            Record::Tick(tick) => ticks.push_back(tick),
            Record::Mark(mark) => marks.push_back((marks.len(), mark)),
            Record::Ui(event) => ui.push_back(event),
            Record::Pointer(row) => pointer.push_back(row),
            Record::NativePad(pad) => {
                native_pad.insert(pad.sim_tick, pad.inputs[0]);
            }
            Record::Device(row) => devices.push((row.host_poll, row.payload)),
            Record::State(row) => devices.push((row.host_poll, format!("s:{}", row.payload))), // T3819, T3873
        }
    }
    if reader.version() >= DEVICE_ROWS_VERSION {
        eprintln!(
            "tsfp-bevy: input replay: {} device rows (format v{}, build {}) feed every input consumer (T3820)",
            devices.len(),
            reader.version(),
            reader.build().unwrap_or("?")
        );
        tsfp_input_intake::set_host(Box::new(IntakeHost));
        tsfp_input_intake::start_replay(devices);
    } else {
        eprintln!(
            "tsfp-bevy: WARN input replay: format v{} has no device rows (T3820): keys, mouse and pad input that native pages read directly (text fields, storage names, Map Maker text, keys and mouse on original pages, options binding capture) is missing; re-record with this build for a faithful replay",
            reader.version()
        );
    }
    let accept_polls: Vec<_> = ticks
        .iter()
        .filter(|tick| {
            tick.inputs[0].is_some_and(|input| input.pressed & tsfp_input::player::bits::A != 0)
        })
        .map(|tick| tick.host_poll)
        .collect();
    let start_polls: Vec<_> = ticks
        .iter()
        .filter(|tick| {
            tick.inputs[0].is_some_and(|input| input.pressed & tsfp_input::player::bits::START != 0)
        })
        .map(|tick| tick.host_poll)
        .collect();
    let echoes = drop_main_menu_echoes(&mut ui);
    tsfp_input_slots::ports::name_replay_ports(&ui); // T3781: players the recording names
    if std::env::var_os("TSFP_POINTER_LOG").is_some() {
        eprintln!(
            "tsfp-bevy: pointer-log: replay rows A-edges={} first-polls={:?} last-polls={:?} START-edges={} start-polls={:?} UI-rows={} pointer-rows={}",
            accept_polls.len(),
            accept_polls.iter().take(8).collect::<Vec<_>>(),
            accept_polls.iter().rev().take(8).collect::<Vec<_>>(),
            start_polls.len(),
            start_polls.iter().take(8).collect::<Vec<_>>(),
            ui.len(),
            pointer.len()
        );
    }
    if echoes != 0 {
        eprintln!(
            "tsfp-bevy: input replay dropped {echoes} main-menu UI row(s) recorded as echoes of a native page's input (T3391)"
        );
    }
    let end_tick = ticks
        .back()
        .map_or(0, |tick| tick.sim_tick)
        .max(ui.back().map_or(0, |event| event.sim_tick));
    let end_poll = ticks
        .back()
        .map_or(0, |tick| tick.host_poll)
        .max(ui.back().map_or(0, |event| event.host_poll));
    if ticks.is_empty() {
        return Err("recording has no simulation ticks".into());
    }
    let first_event_tick = ticks
        .front()
        .map(|tick| tick.sim_tick)
        .into_iter()
        .chain(ui.front().map(|event| event.sim_tick))
        .min()
        .unwrap_or(0);
    {
        let mut table: Vec<(u64, u64)> = ticks
            .iter()
            .map(|tick| (tick.sim_tick, tick.host_poll))
            .chain(ui.iter().map(|event| (event.sim_tick, event.host_poll)))
            .collect();
        table.sort_unstable();
        let mut highest = 0;
        for entry in &mut table {
            highest = highest.max(entry.1);
            entry.1 = highest;
        }
        *lock(&TICK_POLLS) = table;
    }
    let ui_ticks = ui
        .iter()
        .map(|event| (event.page, event.sim_tick))
        .collect();
    let mark_count = marks.len();
    *lock(&PLAYBACK) = Some(Playback {
        ticks,
        native_pad,
        ui,
        ui_ticks,
        pointer_recorded: !pointer.is_empty(),
        accept_polls,
        start_polls,
        pointer,
        sent: SdlReport {
            axes: [Some(0); 6],
            ..SdlReport::default()
        },
        end_tick,
        end_poll,
        first_event_tick,
        timeline_anchor: None,
        hold_after_end: std::env::var_os("TSFP_INPUT_REPLAY_HOLD").is_some(),
        handover_after_end,
        hold_reported: false,
        guest_exclusive_at_final_row: None,
        ui_abandoned_reported: false,
        end_marker_poll: None,
        ticks_replayed: 0,
        marks,
        last_tick_inputs: None,
        button_press_poll: [None; 15],
        poll_latch_wait_reported: 0,
    });
    REPLAY_ENABLED.store(true, Ordering::Release);
    eprintln!(
        "tsfp-bevy: native input replay from {} ({} ticks, {} marks)",
        path.display(),
        lock(&PLAYBACK).as_ref().map_or(0, |p| p.ticks.len()),
        mark_count
    );
    Ok(())
}

pub fn replaying() -> bool {
    REPLAY_ENABLED.load(Ordering::Acquire)
}

/// End native playback and let the live Rust router own subsequent input ticks.
pub fn finish_handover() {
    report_devices();
    tsfp_input_intake::stop();
    lock(&PLAYBACK).take();
    REPLAY_ENABLED.store(false, Ordering::Release);
}

/// Current owner-recording cursor for diagnostic traces. The cursor advances
/// once per recorded gameplay T row and is independent of host poll rate.
pub fn replay_cursor() -> Option<(u64, u64)> {
    let guard = lock(&PLAYBACK);
    let playback = guard.as_ref()?;
    Some((
        playback.ticks_replayed,
        playback
            .last_tick_inputs
            .map_or(0, |(recorded_tick, _)| recorded_tick),
    ))
}

/// T2487: the poll a route-end marker reports: the live poll for a normal end, or one past the
/// last delivered tick row when undelivered UI rows were abandoned (keeps rust_trace's route
/// alignment independent of the abandonment grace).
pub fn route_end_marker_poll() -> u64 {
    lock(&PLAYBACK)
        .as_ref()
        .map_or_else(replay_poll, Playback::end_line_poll)
}

impl Playback {
    /// The host poll an end line (and the T2046 route clock marker) names. T3393: the tick clock
    /// steps the replay poll, so a route end names the recorded end + 1; a hold names the live poll.
    fn end_line_poll(&self) -> u64 {
        tsfp_input_intake::route_boundary::end_line_poll(
            self.end_marker_poll,
            tick_clock() && !self.hold_after_end,
            self.end_poll,
            replay_poll(),
        )
    }
}

pub fn replay_tick(sim_tick: u64) -> Result<ReplayStep, String> {
    let mut guard = lock(&PLAYBACK);
    let Some(playback) = guard.as_mut() else {
        return Err("replay tick requested without a loaded recording".into());
    };
    // T3453: the routing the guest took at its last controller poll, not a fresh read of the page
    // state the guest thread is changing right now.
    let step = playback.step_at_host_poll(
        sim_tick,
        replay_poll(),
        crate::wi::guest_pad_poll_count(),
        !native_ui_owned_last_guest_poll(),
    );
    // A Tick publishes after its events reach the input queue (publish_replay_progress).
    if !matches!(step, Ok(ReplayStep::Tick(..))) {
        publish_progress(playback);
    }
    drop(guard); // T2046: the guest boundary takes the device lock, never under PLAYBACK
    tsfp_input_intake::route_boundary::flush();
    step
}

/// T2487: highest host poll whose recorded rows are all delivered (u64::MAX when none remain).
/// Under fast-forward the host's input poll waits for it (lockstep), so recorded input reaches
/// the guest at its recorded poll however fast the guest runs relative to Bevy's frames.
static REPLAY_DELIVERED_POLL: AtomicU64 = AtomicU64::new(0);
const LOCKSTEP_WAIT: std::time::Duration = std::time::Duration::from_millis(250);
static LOCKSTEP_TIMEOUTS: AtomicU64 = AtomicU64::new(0);

fn delivered_poll(playback: &Playback) -> u64 {
    playback
        .ticks
        .front()
        .map_or(u64::MAX, |tick| tick.host_poll.saturating_sub(1))
        .min(tsfp_input_intake::ready_poll()) // T3820: device rows delivered and processed
}

fn publish_progress(playback: &Playback) {
    REPLAY_DELIVERED_POLL.store(delivered_poll(playback), Ordering::Release);
    crate::shared::PROGRESS.bump(); // T3394: wake the lockstep waiter
}

/// Called by the input router once a Tick's events are in the input queue.
pub fn publish_replay_progress() {
    if let Some(playback) = lock(&PLAYBACK).as_ref() {
        publish_progress(playback);
    }
}

/// TSFP_FAST_FORWARD set and not "0" (the C host's kernel_vtime_enabled rule).
pub fn fast_forward_switch(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

/// T3388: `TSFP_REPLAY_LOCKSTEP=0` turns the replay input lockstep off (diagnostic only).
pub fn lockstep_switch(value: Option<&str>) -> bool {
    value != Some("0")
}

fn lockstep_enabled() -> bool {
    static LOCKSTEP: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LOCKSTEP.get_or_init(|| lockstep_switch(std::env::var("TSFP_REPLAY_LOCKSTEP").ok().as_deref()))
}

/// T2487: host input poll `poll` of a replay waits (bounded) until Bevy delivered its rows.
/// T3388: for paced replays too. Paced, the guest polls at its render rate (uncapped, rule 21)
/// while Bevy delivers rows at most once per Bevy frame, so without the lockstep a paced replay
/// fell behind its recording by a load-dependent amount (MEASURED r5 at load 14: 2771 of 4446
/// tick rows undelivered at the route end).
#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    fn kernel_vtime_host_hold(on: i32);
}

fn host_hold(on: i32) {
    #[cfg(not(tsfp_no_host))]
    // SAFETY: a plain counter update under the C module's own mutex.
    unsafe {
        kernel_vtime_host_hold(on);
    }
    #[cfg(tsfp_no_host)]
    let _ = on;
}

pub fn lockstep_wait(poll: u64) {
    if !replaying() || !lockstep_enabled() {
        return;
    }
    let poll = if tick_clock() { replay_poll() } else { poll };
    // T3393: the guest waits on the host here, not on guest time: no virtual-time idle jump.
    struct Hold;
    impl Hold {
        fn new() -> Self {
            host_hold(1);
            Hold
        }
    }
    impl Drop for Hold {
        fn drop(&mut self) {
            host_hold(0);
        }
    }
    let _hold = Hold::new();
    let deadline = Instant::now() + LOCKSTEP_WAIT;
    let mut seen = crate::shared::PROGRESS.current();
    while REPLAY_DELIVERED_POLL.load(Ordering::Acquire) < poll
        || (tick_clock() && UI_READY_POLL.load(Ordering::Acquire) <= poll)
        || (tick_clock()
            && UI_REPLAY_ACTIVE.load(Ordering::Acquire)
            && UI_TICK_DONE.load(Ordering::Acquire) < crate::wi::simulation_tick())
    {
        let now = Instant::now();
        if now >= deadline {
            let timeouts = LOCKSTEP_TIMEOUTS.fetch_add(1, Ordering::Relaxed) + 1;
            if timeouts.is_power_of_two() {
                eprintln!(
                    "tsfp-bevy: replay lockstep: poll {poll} proceeded after {LOCKSTEP_WAIT:?} without its recorded rows ({timeouts} timeouts)"
                );
            }
            return;
        }
        // T3394: sleep until Bevy delivers (it bumps the gate), not a 50 us poll loop.
        seen = crate::shared::PROGRESS.wait_past(
            seen,
            (deadline - now).min(std::time::Duration::from_millis(5)),
        );
    }
}

/// T3437: the guest's own controller poll waits on the same lockstep before it drains the input
/// queue. `tsfp_wi_pump` runs on the presenter thread at the modelled vblank, after the guest
/// already read its pad for the tick, so the pump lockstep only bounded the delivery to one frame
/// late: whether a tick's recorded rows reached the guest in that tick or the next one depended on
/// how fast Bevy ran (MEASURED story-level1-20261018: right-stick values one poll apart at pad poll
/// 3970 in 5 of 6 off/off pairs, then a different look yaw for the rest of the run). Waiting here,
/// on the guest thread and before the drain, delivers exactly the rows up to the current tick's
/// recorded poll at every guest poll. Bevy cannot run ahead: rows of a later tick become due only
/// after the guest's own thread publishes that tick.
pub fn guest_poll_sync() {
    if replaying() {
        if lockstep_enabled() {
            crate::shared::PROGRESS.bump(); // wake a headless Bevy waiting on the gate
            lockstep_wait(crate::wi::host_poll_index());
        }
        // T3453: the routing decision belongs to this guest poll (guest thread, program order
        // after the frame hooks that publish the live page), not to Bevy's update.
        GUEST_POLL_NATIVE_UI.store(crate::ui_live::native_ui_owns_input(), Ordering::Release);
    }
    deliver_guest_bound();
}

/// T3453: a Rust page owned input at the guest's last controller poll of a replay.
static GUEST_POLL_NATIVE_UI: AtomicBool = AtomicBool::new(false);

pub fn native_ui_owned_last_guest_poll() -> bool {
    GUEST_POLL_NATIVE_UI.load(Ordering::Acquire)
}

/// T3453: replayed tick-row events (pad shim of the recorded mouse, keyboard and pad input) in
/// recorded order, waiting for the guest's next controller poll. The input router used to feed
/// them into the input queue on the Bevy thread and to drop them there when a Rust page owned
/// input, deciding on whatever page state the guest thread had published at that moment, so a
/// row at a page transition went either way. MEASURED mapmaker-editor-20261018 (with the shim
/// clock bug of `mapmaker_ui_live::legacy_select_mode_cursor`): the editor menu (0x313010,
/// native input) went live after recorded row 2450, 2454, 2475 or 2482 in four runs, and the
/// mouse-look rows before that point reached the guest's right stick while later ones were
/// dropped, so the pad table and `mapedit_obj` differed from guest poll 856 on.
static GUEST_BOUND: Mutex<VecDeque<ShimEvent>> = Mutex::new(VecDeque::new());
static GUEST_BOUND_PENDING: AtomicBool = AtomicBool::new(false);

/// Bevy thread: queue a replayed tick's events for the guest's next controller poll.
pub fn queue_for_guest_poll(events: Vec<ShimEvent>) {
    if events.is_empty() {
        return;
    }
    lock(&GUEST_BOUND).extend(events);
    GUEST_BOUND_PENDING.store(true, Ordering::Release);
}

/// T3799: recorded host poll up to which a replayed pad A press is dropped, because a replay shim already acted on
/// that recorded A edge natively (0: none).
static ACCEPT_EDGE_CONSUMED_UNTIL: AtomicU64 = AtomicU64::new(0);

pub fn consume_accept_edge_until(host_poll: u64) {
    ACCEPT_EDGE_CONSUMED_UNTIL.store(host_poll, Ordering::Release);
}

/// Take the queued events: every one goes to the guest, or none when a Rust page owns input. An A press the shim
/// consumed (`consume_accept_edge_until`) is dropped; its release is harmless and passes.
fn take_guest_bound(queue: &mut VecDeque<ShimEvent>, native_ui_owns_input: bool) -> Vec<ShimEvent> {
    let until = ACCEPT_EDGE_CONSUMED_UNTIL.load(Ordering::Acquire);
    let poll = replay_poll();
    queue.retain(|e| !consumed_accept(*e, poll, until));
    crate::input_router::replay_shims_for_page(queue.drain(..).collect(), native_ui_owns_input)
}

/// Whether `event` is the A press of an edge a replay shim already consumed (`until` 0: none).
fn consumed_accept(event: ShimEvent, poll: u64, until: u64) -> bool {
    event == ShimEvent::ButtonDown(0) && until != 0 && poll <= until
}

/// Move the queued replay events into the input queue the guest's controller poll drains next.
/// Guest thread (`guest_poll_sync`), and the live path after a replay handover so no queued row
/// is overtaken by live input.
pub fn deliver_guest_bound() {
    if !GUEST_BOUND_PENDING.swap(false, Ordering::AcqRel) {
        return;
    }
    let native = replaying() && native_ui_owned_last_guest_poll();
    let events = take_guest_bound(&mut lock(&GUEST_BOUND), native);
    if std::env::var_os("TSFP_REPLAY_INPUT_LOG").is_some() {
        eprintln!(
            "replay-deliver: guest-polls {} tick {} replay-poll {} native-ui {native} events {events:?}",
            crate::wi::guest_pad_poll_count(),
            crate::wi::simulation_tick(),
            replay_poll(),
        );
    }
    if events.is_empty() {
        return;
    }
    let mut wi = lock(&crate::shared::SHARED.wi);
    for event in events {
        wi.feed(crate::input_router::shim_raw_event(event));
    }
}

/// Peek the recorded player snapshot for native UI pages before the guest input tick consumes it.
/// The last snapshot remains visible through repeated render frames of the same fixed tick.
pub fn replay_player_input(sim_tick: u64) -> Option<PlayerInput> {
    let mut guard = lock(&PLAYBACK);
    let playback = guard.as_mut()?;
    playback.player_input(sim_tick, 0)
}

/// Whether this recording has semantic UI events for the active page.
/// Older recordings stored those actions only as gameplay `T` snapshots; the
/// Bevy page handler can consume those snapshots directly as native commands.
/// Whether a semantic row for this page and recorded tick already represents
/// the input. This prevents duplicate delivery while allowing sparse legacy v4
/// snapshots to reach native page handlers.
pub fn replay_has_ui_event(active_page: Option<u32>, live_tick: u64) -> bool {
    let mut guard = lock(&PLAYBACK);
    let Some(playback) = guard.as_mut() else {
        return false;
    };
    let recorded_tick = playback.recorded_tick(live_tick);
    ui_page_has_event_at(&playback.ui_ticks, active_page, recorded_tick)
}

/// Return due UI events for the page that owns this host poll. Rows remain in replay order while
/// no native page can consume them, allowing the guest poll path to receive transition-gap rows.
/// Profile selection and its text editor share a builder, so text events still wait for the
/// text-field handler instead of being drained by the selection page.
pub fn replay_ui_for_pages(
    host_poll: u64,
    active_page: Option<u32>,
    active_text_page: Option<u32>,
) -> Result<Vec<Ui>, String> {
    let mut guard = lock(&PLAYBACK);
    let Some(playback) = guard.as_mut() else {
        return Err("replay UI requested without a loaded recording".into());
    };
    if std::env::var_os("TSFP_REPLAY_INPUT_LOG").is_some() {
        static LOGGED_WAITING_ROWS: std::sync::OnceLock<Mutex<HashSet<(u64, u32)>>> =
            std::sync::OnceLock::new();
        let mut logged = lock(LOGGED_WAITING_ROWS.get_or_init(|| Mutex::new(HashSet::new())));
        for event in playback
            .ui
            .iter()
            .filter(|event| event.host_poll <= host_poll)
        {
            let page_ready =
                active_page.is_some_and(|page| ui_event_is_ready(event, page, active_text_page));
            if !page_ready && logged.insert((event.host_poll, event.page)) {
                let guest_top = crate::ui_overlay::top_live_page().map(|page| page.builder);
                eprintln!(
                    "replay-ui: waiting row poll {} device UI context {} player {} page {:#x} kind {} event {:?}; live page {:?}, guest top {:?}, text page {:?}",
                    event.host_poll,
                    event.context,
                    event.player,
                    event.page,
                    ui_event_kind(&event.event),
                    event.event,
                    active_page,
                    guest_top,
                    active_text_page,
                );
            }
        }
    }
    let events = playback.ui_events_for_pages(host_poll, 0, active_page, active_text_page)?;
    if std::env::var_os("TSFP_REPLAY_INPUT_LOG").is_some() {
        for event in &events {
            if active_page.is_some_and(|page| ui_event_is_ready(event, page, active_text_page)) {
                eprintln!(
                    "replay-ui: delivered row poll {} page {:#x} kind {} to live page {:?}, text page {:?}",
                    event.host_poll,
                    event.page,
                    ui_event_kind(&event.event),
                    active_page,
                    active_text_page,
                );
            } else {
                eprintln!(
                    "replay-ui: queued row poll {} page {:#x} kind {} for that registered Rust page's command queue (live page {:?})",
                    event.host_poll,
                    event.page,
                    ui_event_kind(&event.event),
                    active_page,
                );
            }
        }
    }
    // Preserve route order: a later row must not release a guest tick while
    // the queue head still waits for its current Rust owner.
    let ready = active_page
        .and_then(|page| {
            playback
                .ui
                .front()
                .filter(|event| ui_event_is_ready(event, page, active_text_page))
                .map(|event| event.host_poll)
        })
        .unwrap_or(u64::MAX);
    UI_READY_POLL.store(ready, Ordering::Release);
    crate::shared::PROGRESS.bump();
    Ok(events)
}

/// T3393: last simulation tick the native UI replay (ui_live) processed, and whether it runs at
/// all. Under the tick clock the guest's lockstep waits for it, so every recorded tick reaches the
/// native pages exactly once and before the guest's next frame, whatever Bevy's update rate.
static UI_TICK_DONE: AtomicU64 = AtomicU64::new(0);
static UI_REPLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn ui_tick_processed(tick: u64) {
    UI_REPLAY_ACTIVE.store(true, Ordering::Release);
    UI_TICK_DONE.fetch_max(tick, Ordering::AcqRel);
    crate::shared::PROGRESS.bump();
}

/// T3393: host poll of the earliest queued UI row whose page is live (u64::MAX when none).
static UI_READY_POLL: AtomicU64 = AtomicU64::new(u64::MAX);

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

pub fn record_tick(
    sim_tick: u64,
    host_poll: u64,
    inputs: [Option<PlayerInput>; 4],
) -> Result<(), String> {
    let mut guard = lock(&CAPTURE);
    let capture = guard
        .as_mut()
        .ok_or_else(|| "recording was enabled without an open file".to_string())?;
    capture
        .writer
        .tick(&Tick {
            sim_tick,
            host_poll,
            wall_ns: elapsed_ns(capture.started),
            inputs,
        })
        .map_err(|error| error.to_string())
}

pub fn state_digest(sim_tick: u64, host_poll: u64, replaying: bool) {
    static CLOCK: tsfp_input::state_digest::Clock = tsfp_input::state_digest::Clock::new(); // T3819
    if (enabled() || replaying) && CLOCK.due(host_poll, crate::hud_live::guest_read32) {
        let payload = tsfp_input::state_digest::payload(crate::hud_live::guest_read32);
        let mut capture = lock(&CAPTURE);
        match capture.as_mut().filter(|_| !replaying) {
            Some(c) => drop(c.writer.state(&Payload {
                sim_tick,
                host_poll,
                wall_ns: elapsed_ns(c.started),
                payload,
            })),
            None => eprintln!("replay-state poll={host_poll} tick={sim_tick} {payload}"),
        }
    }
}

/// T3592: store the physical pad for a tick a native page owned (the `T` row is `-` then), so a press
/// that crosses from the page to the guest (a cutscene skip) is in the file.
pub fn record_native_pad(
    sim_tick: u64,
    host_poll: u64,
    inputs: [Option<PlayerInput>; 4],
) -> Result<(), String> {
    let mut guard = lock(&CAPTURE);
    let capture = guard
        .as_mut()
        .ok_or_else(|| "recording was enabled without an open file".to_string())?;
    capture
        .writer
        .native_pad(&Tick {
            sim_tick,
            host_poll,
            wall_ns: elapsed_ns(capture.started),
            inputs,
        })
        .map_err(|error| error.to_string())
}

pub fn record_ui(page: u32, context: u8, player: u8, event: RecordedUiEvent) -> Result<(), String> {
    let mut guard = lock(&CAPTURE);
    let capture = guard
        .as_mut()
        .ok_or_else(|| "UI event recorded without an open input recording".to_string())?;
    capture
        .writer
        .ui(&Ui {
            sim_tick: crate::wi::simulation_tick(),
            host_poll: crate::wi::host_poll_index(),
            wall_ns: elapsed_ns(capture.started),
            context,
            player,
            page,
            event,
        })
        .map_err(|error| error.to_string())
}

/// T3392: record the mouse pointer (design grid position or `None` outside the window, held button
/// mask) at this guest frame boundary. Written only when it differs from the previous row.
pub fn record_pointer(position: Option<(f32, f32)>, buttons: u8) -> Result<(), String> {
    static LAST: Mutex<Option<(Option<(u32, u32)>, u8)>> = Mutex::new(None);
    let key = (position.map(|(x, y)| (x.to_bits(), y.to_bits())), buttons);
    {
        let mut last = lock(&LAST);
        if *last == Some(key) || (last.is_none() && position.is_none() && buttons == 0) {
            return Ok(());
        }
        *last = Some(key);
    }
    let mut guard = lock(&CAPTURE);
    let capture = guard
        .as_mut()
        .ok_or_else(|| "pointer recorded without an open input recording".to_string())?;
    capture
        .writer
        .pointer(&Pointer {
            sim_tick: crate::wi::simulation_tick(),
            host_poll: crate::wi::host_poll_index(),
            wall_ns: elapsed_ns(capture.started),
            buttons,
            position,
        })
        .map_err(|error| error.to_string())
}

/// T3392: whether the loaded recording carries pointer rows (a replay then owns the Map Maker pointer).
pub fn replay_has_pointer_rows() -> bool {
    lock(&PLAYBACK)
        .as_ref()
        .is_some_and(|playback| playback.pointer_recorded)
}

/// T3392: whether the recording holds an A press edge of player 0 within `before` polls ahead of and
/// `after` polls behind `host_poll` (the legacy pointer shim acts a few polls before the guest reads it).
pub fn replay_accept_edge_near(host_poll: u64, before: u64, after: u64) -> bool {
    lock(&PLAYBACK)
        .as_ref()
        .is_some_and(|playback| accept_edge_near(&playback.accept_polls, host_poll, before, after))
}

/// Whether the sorted recorded edge polls hold one in `host_poll - after ..= host_poll + before`.
fn accept_edge_near(accept_polls: &[u64], host_poll: u64, before: u64, after: u64) -> bool {
    let start = accept_polls.partition_point(|poll| *poll < host_poll.saturating_sub(after));
    accept_polls
        .get(start)
        .is_some_and(|poll| *poll <= host_poll.saturating_add(before))
}

/// T3395: whether player 0 has a recorded Start press at exactly `host_poll`.
pub fn replay_start_edge_at(host_poll: u64) -> bool {
    lock(&PLAYBACK)
        .as_ref()
        .is_some_and(|playback| playback.start_polls.binary_search(&host_poll).is_ok())
}

/// T3392: the newest recorded pointer sample due at or before `host_poll` (outer `None`: none due, the
/// previous sample stays; inner `None`: the cursor was outside the window). Rows are consumed.
pub fn replay_pointer(host_poll: u64) -> Option<Option<(f32, f32)>> {
    let mut guard = lock(&PLAYBACK);
    let playback = guard.as_mut()?;
    let mut latest = None;
    while playback
        .pointer
        .front()
        .is_some_and(|row| row.host_poll <= host_poll)
    {
        latest = playback.pointer.pop_front().map(|row| row.position);
    }
    latest
}

pub fn mark(label: &str, sim_tick: u64, host_poll: u64) -> Result<(), String> {
    let mut guard = lock(&CAPTURE);
    let capture = guard
        .as_mut()
        .ok_or_else(|| "mark requested without an active recording".to_string())?;
    capture
        .writer
        .mark(&Mark {
            sim_tick,
            host_poll,
            wall_ns: elapsed_ns(capture.started),
            label: label.to_string(),
        })
        .map_err(|error| error.to_string())
}

fn report_devices() {
    if let Some(line) = tsfp_input_intake::report() {
        eprintln!("tsfp-bevy: input replay {line}");
    }
}

pub fn finish() -> Result<(), String> {
    report_devices();
    tsfp_input_intake::stop();
    ENABLED.store(false, Ordering::Release);
    let Some(capture) = lock(&CAPTURE).take() else {
        return Ok(());
    };
    capture
        .writer
        .finish()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn mark_hotkey(key: bevy::input::keyboard::KeyCode) -> bool {
    if key != tsfp_input_intake::MARK_HOTKEY || !enabled() {
        return false;
    }
    let tick = crate::wi::simulation_tick();
    let poll = crate::wi::host_poll_index();
    if let Err(error) = mark("user", tick, poll) {
        eprintln!("tsfp-bevy: input recording mark failed: {error}");
    } else {
        eprintln!("tsfp-bevy: input recording mark at sim tick {tick}, host poll {poll}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    use tsfp_input::player::{AssistMask, Look, MenuIntent, SourceKind, Vec2, bits};

    const SDL_A_BUTTON: u16 = 1;

    #[test]
    fn a_shim_consumed_accept_press_is_not_delivered_again() {
        // T3799 (I-T3799-2): the Select Mode shim acted on the recorded A edge; the guest must not get it again.
        let down = ShimEvent::ButtonDown(0);
        assert!(consumed_accept(down, 2731, 2732));
        assert!(!consumed_accept(down, 2733, 2732), "a later A press passes");
        assert!(!consumed_accept(down, 0, 0), "nothing consumed");
        assert!(
            !consumed_accept(ShimEvent::ButtonUp(0), 2731, 2732),
            "the release passes"
        );
        assert!(
            !consumed_accept(ShimEvent::ButtonDown(1), 2731, 2732),
            "other buttons pass"
        );
    }

    fn playback(inputs: [Option<PlayerInput>; 4]) -> Playback {
        Playback {
            ticks: VecDeque::from([Tick {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                inputs,
            }]),
            native_pad: HashMap::new(),
            ui: VecDeque::new(),
            ui_ticks: HashSet::new(),
            pointer: VecDeque::new(),
            pointer_recorded: false,
            accept_polls: Vec::new(),
            start_polls: Vec::new(),
            sent: SdlReport {
                axes: [Some(0); 6],
                ..SdlReport::default()
            },
            end_tick: 1,
            end_poll: 4,
            first_event_tick: 1,
            timeline_anchor: None,
            hold_after_end: false,
            handover_after_end: false,
            hold_reported: false,
            guest_exclusive_at_final_row: None,
            ui_abandoned_reported: false,
            end_marker_poll: None,
            ticks_replayed: 0,
            marks: VecDeque::new(),
            last_tick_inputs: None,
            button_press_poll: [None; 15],
            poll_latch_wait_reported: 0,
        }
    }

    #[test]
    fn final_row_guest_boundary_ignores_later_completion_poll() {
        let mut replay = playback([None; 4]);
        replay.step_with_poll(1, 0, 6591).unwrap();
        assert_eq!(replay.guest_exclusive_at_final_row, Some(6591));
        // Avoid process-global marker state while exercising the completion path.
        replay.hold_reported = true;
        replay.step_with_poll(2, 0, 6592).unwrap();
        assert_eq!(replay.guest_exclusive_at_final_row, Some(6591));
    }

    #[test]
    fn native_pad_rows_reach_the_guest_on_dash_ticks() {
        let start = PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held: bits::START,
            pressed: bits::START,
            released: 0,
            move_axis: Vec2::ZERO,
            look: Look::Stick(Vec2::ZERO),
            triggers: [0.0; 2],
            menu: MenuIntent::default(),
            assists: AssistMask::NONE,
        };
        let mut replay = playback([None; 4]);
        replay.native_pad.insert(1, Some(start));
        replay.step_with_poll(1, u64::MAX, 0).unwrap();
        assert_ne!(
            replay.sent.buttons, 0,
            "G row press must reach the guest pad"
        );
    }

    #[test]
    fn native_replay_handover_precedes_neutral_hold_at_route_end() {
        let mut replay = playback([None; 4]);
        replay.ticks.clear();
        replay.end_tick = 0;
        replay.hold_after_end = true;
        replay.handover_after_end = true;
        assert!(matches!(
            replay.step_with_delivery(1, u64::MAX, 0, false).unwrap(),
            ReplayStep::Handover
        ));
    }

    #[test]
    fn replay_keeps_guest_press_until_polled_without_blocking_later_rows() {
        const SDL_A_BUTTON: u16 = 1;
        const SDL_START_BUTTON: u16 = 1 << 6;

        let down = PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held: bits::START,
            pressed: bits::START,
            released: 0,
            move_axis: Vec2::ZERO,
            look: Look::Stick(Vec2::ZERO),
            triggers: [0.0; 2],
            menu: MenuIntent::default(),
            assists: AssistMask::NONE,
        };
        let up = PlayerInput {
            step: 2,
            held: 0,
            pressed: 0,
            released: bits::START,
            ..down
        };
        let later = PlayerInput {
            step: 3,
            held: bits::A,
            pressed: bits::A,
            released: 0,
            ..down
        };
        let mut replay = playback([Some(down), None, None, None]);
        replay.ticks = VecDeque::from([
            Tick {
                sim_tick: 1,
                host_poll: 1,
                wall_ns: 20,
                inputs: [Some(down), None, None, None],
            },
            Tick {
                sim_tick: 2,
                host_poll: 2,
                wall_ns: 30,
                inputs: [Some(up), None, None, None],
            },
            Tick {
                sim_tick: 3,
                host_poll: 3,
                wall_ns: 40,
                inputs: [Some(later), None, None, None],
            },
            Tick {
                sim_tick: 4,
                host_poll: 4,
                wall_ns: 50,
                inputs: [None; 4],
            },
        ]);
        replay.end_tick = 4;

        assert!(matches!(
            replay.step_with_poll(1, u64::MAX, 10).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_ne!(replay.sent.buttons, 0);
        assert!(matches!(
            replay.step_with_poll(2, u64::MAX, 10).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_ne!(replay.sent.buttons & SDL_START_BUTTON, 0);
        assert_eq!(replay.ticks.len(), 2);
        assert!(matches!(
            replay.step_with_poll(3, u64::MAX, 10).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_eq!(replay.sent.buttons & SDL_START_BUTTON, SDL_START_BUTTON);
        assert_eq!(replay.sent.buttons & SDL_A_BUTTON, SDL_A_BUTTON);
        assert!(matches!(
            replay.step_with_poll(4, u64::MAX, 11).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_eq!(replay.sent.buttons, 0);
    }

    #[test]
    fn replay_native_ui_press_does_not_wait_for_a_guest_pad_poll() {
        let down = PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held: bits::START,
            pressed: bits::START,
            released: 0,
            move_axis: Vec2::ZERO,
            look: Look::Stick(Vec2::ZERO),
            triggers: [0.0; 2],
            menu: MenuIntent::default(),
            assists: AssistMask::NONE,
        };
        let up = PlayerInput {
            step: 2,
            held: 0,
            pressed: 0,
            released: bits::START,
            ..down
        };
        let mut replay = playback([Some(down), None, None, None]);
        replay.ticks = VecDeque::from([
            Tick {
                sim_tick: 1,
                host_poll: 1,
                wall_ns: 20,
                inputs: [Some(down), None, None, None],
            },
            Tick {
                sim_tick: 2,
                host_poll: 2,
                wall_ns: 30,
                inputs: [Some(up), None, None, None],
            },
        ]);
        replay.end_tick = 2;

        assert!(matches!(
            replay.step_with_delivery(1, u64::MAX, 10, false).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert!(matches!(
            replay.step_with_delivery(2, u64::MAX, 10, false).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_eq!(replay.ticks_replayed, 2);
        assert_eq!(replay.sent.buttons, 0);
        assert_eq!(replay.button_press_poll[0], None);
    }

    #[test]
    fn same_page_same_tick_ui_events_suppress_legacy_tick_fallback() {
        let events = HashSet::from([(0x194980, 17), (0, 19)]);
        assert!(ui_page_has_event_at(&events, Some(0x194980), 17));
        assert!(!ui_page_has_event_at(&events, Some(0x194980), 18));
        assert!(!ui_page_has_event_at(&events, Some(0x1B2D40), 17));
        assert!(!ui_page_has_event_at(&events, None, 19));
        assert!(ui_page_has_event_at(
            &events,
            Some(tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32),
            19
        ));
    }

    fn input(held: u32) -> PlayerInput {
        PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held,
            pressed: held,
            released: 0,
            move_axis: Vec2::new(0.0, 0.0),
            look: Look::Stick(Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: MenuIntent::default(),
            assists: AssistMask::NONE,
        }
    }

    #[test]
    fn replay_requires_the_recorded_unlock_policy() {
        assert!(check_unlock_policy(false, false).is_ok());
        assert!(check_unlock_policy(true, true).is_ok());
        assert!(check_unlock_policy(false, true).is_err());
        assert!(check_unlock_policy(true, false).is_err());
    }

    #[test]
    fn replay_waits_for_recorded_tick_and_injects_then_releases_pad_state() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.end_tick = 2;
        replay.ticks.push_back(Tick {
            sim_tick: 2,
            host_poll: 8,
            wall_ns: 50,
            inputs: [None; 4],
        });
        assert!(matches!(
            replay.step(0, u64::MAX).unwrap(),
            ReplayStep::Wait
        ));
        let ReplayStep::Tick(inputs, events) = replay.step(1, u64::MAX).unwrap() else {
            panic!("expected tick")
        };
        assert_eq!(inputs[0].unwrap().held, bits::A);
        assert!(events.contains(&ShimEvent::ButtonDown(0)));
        let ReplayStep::Tick(_, events) = replay.step(2, u64::MAX).unwrap() else {
            panic!("expected release tick")
        };
        assert!(events.contains(&ShimEvent::ButtonUp(0)));
        assert!(matches!(replay.step(3, u64::MAX).unwrap(), ReplayStep::End));
    }

    #[test]
    fn replay_gameplay_rows_follow_recorded_host_polls_while_simulation_is_paused() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.ticks = VecDeque::from([
            Tick {
                sim_tick: 600,
                host_poll: 40,
                wall_ns: 9_000_000_000,
                inputs: [Some(input(bits::A)), None, None, None],
            },
            Tick {
                sim_tick: 692,
                host_poll: 48,
                wall_ns: 20_000_000_000,
                inputs: [None; 4],
            },
        ]);
        replay.end_tick = 692;
        replay.end_poll = 48;
        replay.first_event_tick = 600;

        assert!(matches!(
            replay.step_at_host_poll(0, 39, 0, true).unwrap(),
            ReplayStep::Wait
        ));
        assert!(matches!(
            replay.step_at_host_poll(0, 40, 0, true).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_eq!(replay.sent.buttons, SDL_A_BUTTON);
        assert!(matches!(
            replay.step_at_host_poll(0, 47, 0, true).unwrap(),
            ReplayStep::Wait
        ));
        assert!(matches!(
            replay.step_at_host_poll(0, 48, 1, true).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert_eq!(replay.sent.buttons, 0);
        assert!(matches!(
            replay.step_at_host_poll(0, 49, 0, true).unwrap(),
            ReplayStep::End
        ));
    }

    #[test]
    fn replay_uses_simulation_ticks_without_wall_clock_gating() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.end_tick = 2;
        replay.ticks.push_back(Tick {
            sim_tick: 2,
            host_poll: 906,
            wall_ns: 6_200_000_000,
            inputs: [None; 4],
        });

        assert!(matches!(replay.step(1, 0).unwrap(), ReplayStep::Tick(_, _)));
        assert!(matches!(
            replay.step(2, 1_000_000_000).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert!(matches!(
            replay.step(361, 6_199_999_980).unwrap(),
            ReplayStep::End
        ));
    }

    #[test]
    fn replay_waits_for_fixed_tick_instead_of_catching_up_by_wall_clock() {
        let mut replay = playback([None; 4]);
        replay.ticks = VecDeque::from([
            Tick {
                sim_tick: 600,
                host_poll: 2400,
                wall_ns: 10_000_000_000,
                inputs: [None; 4],
            },
            Tick {
                sim_tick: 692,
                host_poll: 2683,
                wall_ns: 20_190_000_000,
                inputs: [Some(input(0x80)), None, None, None],
            },
        ]);
        replay.end_tick = 692;
        replay.first_event_tick = 600;

        assert!(
            matches!(replay.step(0, u64::MAX).unwrap(), ReplayStep::Wait),
            "the boot clock at zero must not consume future gameplay rows"
        );
        assert!(matches!(
            replay.step(600, 10_000_000_000).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        assert!(matches!(
            replay.step(600, u64::MAX).unwrap(),
            ReplayStep::Wait
        ));
        let ReplayStep::Tick(inputs, _) = replay.step(692, 0).unwrap() else {
            panic!("input must become due when the recorded fixed tick is reached");
        };
        assert_eq!(inputs[0].map(|input| input.held), Some(0x80));
        assert_eq!(
            replay.player_input(692, 0).map(|input| input.held),
            Some(0x80),
            "the forward anchor keeps the live tick mapped to the replayed row"
        );
    }

    #[test]
    fn main_menu_rows_sharing_a_poll_with_another_page_are_dropped_as_echoes() {
        let main = tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32;
        let row = |host_poll, page, event| Ui {
            sim_tick: host_poll,
            host_poll,
            wall_ns: 0,
            context: 0,
            player: 0,
            page,
            event,
        };
        let mut ui = VecDeque::from([
            row(10, main, RecordedUiEvent::Activate(0)),
            row(10, 0x2d2350, RecordedUiEvent::Start),
            row(20, main, RecordedUiEvent::Activate(4)),
            row(30, 0x194980, RecordedUiEvent::Back),
        ]);
        assert_eq!(drop_main_menu_echoes(&mut ui), 1);
        let kept: Vec<_> = ui
            .iter()
            .map(|event| (event.host_poll, event.page))
            .collect();
        assert_eq!(kept, [(10, 0x2d2350), (20, main), (30, 0x194980)]);
    }

    /// T3772: profile page rows of player 2 and page action rows with a zero delta (Delete, Join on port 0) replay;
    /// before, they never became ready and held the whole UI replay queue at its head.
    #[test]
    fn profile_rows_of_every_player_and_zero_delta_page_actions_are_ready() {
        use tsfp_ui_input::page_intents::{RECORDED_DELETE_LIST, RECORDED_JOIN_LIST};
        let profile = tsfp_ui_input::profile_page::BUILDER;
        let row = |player, page, event| Ui {
            sim_tick: 1,
            host_poll: 1,
            wall_ns: 0,
            context: 0,
            player,
            page,
            event,
        };
        let delete = RecordedUiEvent::Scroll(RECORDED_DELETE_LIST, 0);
        assert!(ui_event_is_ready(&row(0, profile, delete), profile, None));
        let join = RecordedUiEvent::Scroll(RECORDED_JOIN_LIST, 0);
        assert!(ui_event_is_ready(&row(0, profile, join), profile, None));
        let start = RecordedUiEvent::Start;
        assert!(ui_event_is_ready(
            &row(1, profile, start.clone()),
            profile,
            None
        ));
        assert!(!ui_event_is_ready(&row(1, 0x2d2350, start), 0x2d2350, None));
        assert!(!ui_event_is_ready(
            &row(0, profile, RecordedUiEvent::Scroll(0, 0)),
            profile,
            None
        ));
    }

    /// T3787 (I-T3787-2): rows recorded on a `NATIVE_INPUT_PAGES` page (Arcade setup category 0x2D5E70) replay
    /// natively, as live input routed them; they waited forever and no Arcade replay reached gameplay.
    #[test]
    fn native_input_page_rows_are_ready() {
        let setup = 0x2D_5E70;
        let row = Ui {
            sim_tick: 1,
            host_poll: 1,
            wall_ns: 0,
            context: 0,
            player: 0,
            page: setup,
            event: RecordedUiEvent::Scroll(0, -1),
        };
        for (page, _) in crate::ui_live::NATIVE_INPUT_PAGES {
            assert!(native_ui_page_has_handler(*page), "{page:#x}");
        }
        assert!(ui_event_is_ready(&row, setup, None));
        assert!(native_ui_page_has_handler(
            tsfp_ui_model::frontend::MAIN_MENU_BUILDER as u32
        ));
        assert!(!native_ui_page_has_handler(0x0012_3456));
    }

    #[test]
    fn replay_ui_waits_for_its_recorded_host_frame_even_when_tick_is_held() {
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ui = VecDeque::from([Ui {
            sim_tick: 1,
            host_poll: 906,
            wall_ns: 6_200_000_000,
            context: 0,
            player: 0,
            page: 0x2d0be0,
            event: RecordedUiEvent::Start,
        }]);

        assert!(
            replay
                .ui_events(905, u64::MAX, Some(0x2d0be0))
                .unwrap()
                .is_empty()
        );
        assert_eq!(replay.ui_events(906, 0, Some(0x2d0be0)).unwrap().len(), 1);
    }

    #[test]
    fn replay_abandons_undelivered_ui_rows_after_the_route_end_grace() {
        let mut replay = playback([None; 4]);
        replay.ticks.clear();
        replay.end_poll = 7215;
        replay.ui = VecDeque::from([Ui {
            sim_tick: 4275,
            host_poll: 6877,
            wall_ns: 1,
            context: 0,
            player: 0,
            page: 0x2d0be0,
            event: RecordedUiEvent::Start,
        }]);
        let at = |replay: &mut Playback, poll: u64| {
            replay
                .step_scheduled(1, Some(poll), 0, false, None, None)
                .unwrap()
        };
        assert!(matches!(at(&mut replay, 7216), ReplayStep::Wait));
        assert!(matches!(
            at(&mut replay, 7215 + UI_ROWS_END_GRACE_POLLS),
            ReplayStep::Wait
        ));
        assert!(matches!(
            at(&mut replay, 7216 + UI_ROWS_END_GRACE_POLLS),
            ReplayStep::End
        ));
        assert!(replay.ui_abandoned_reported);
        assert_eq!(replay.end_marker_poll, Some(7216));
    }

    #[test]
    fn t3607_marks_are_reported_once_when_their_tick_is_reached() {
        let mut replay = playback([None; 4]);
        for (index, tick) in [(0, 0), (1, 5), (2, 9)] {
            replay.marks.push_back((
                index,
                Mark {
                    sim_tick: tick,
                    host_poll: tick * 2,
                    wall_ns: 0,
                    label: "user".into(),
                },
            ));
        }
        replay.report_marks_reached(4, 40);
        assert_eq!(
            replay.marks.len(),
            2,
            "the tick 0 mark is reported, the others wait"
        );
        replay.report_marks_reached(9, 90);
        assert!(replay.marks.is_empty());
    }

    #[test]
    fn lockstep_delivered_poll_is_the_poll_before_the_next_queued_row() {
        let mut replay = playback([None; 4]);
        assert_eq!(delivered_poll(&replay), 3); // the fixture's row is due at host poll 4
        replay.ticks.clear();
        assert_eq!(delivered_poll(&replay), u64::MAX);
        assert!(fast_forward_switch(Some("1")));
        assert!(!fast_forward_switch(Some("0")));
        assert!(!fast_forward_switch(None));
        assert!(lockstep_switch(None));
        assert!(lockstep_switch(Some("1")));
        assert!(!lockstep_switch(Some("0")));
    }

    #[test]
    fn replay_abandons_stale_tick_rows_after_the_route_end_grace() {
        let mut replay = playback([None; 4]);
        replay.end_poll = 7215;
        replay.end_tick = 4446;
        replay.ticks = VecDeque::from([Tick {
            sim_tick: 4446,
            host_poll: 7215,
            wall_ns: 1,
            inputs: [None; 4],
        }]);
        let poll = 7216 + UI_ROWS_END_GRACE_POLLS;
        assert!(matches!(
            replay
                .step_scheduled(1, Some(poll), 0, false, None, None)
                .unwrap(),
            ReplayStep::End
        ));
        assert!(replay.ticks.is_empty());
        assert_eq!(replay.end_marker_poll, Some(7216));
    }

    #[test]
    fn replay_offsets_recording_when_live_start_has_already_passed_first_event() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.ticks = VecDeque::from([
            Tick {
                sim_tick: 6,
                host_poll: 4,
                wall_ns: 20,
                inputs: [Some(input(bits::A)), None, None, None],
            },
            Tick {
                sim_tick: 7,
                host_poll: 8,
                wall_ns: 50,
                inputs: [None; 4],
            },
        ]);
        replay.first_event_tick = 6;
        replay.end_tick = 7;
        replay.ui = VecDeque::from([Ui {
            sim_tick: 6,
            host_poll: 4,
            wall_ns: 21,
            context: 0,
            player: 0,
            page: 0x2d0be0,
            event: RecordedUiEvent::Focus(7),
        }]);

        assert!(replay.ui_events(0, 0, Some(0x2d0be0)).unwrap().is_empty());
        assert_eq!(replay.ui_events(4, 1, Some(0x2d0be0)).unwrap().len(), 1);

        let ReplayStep::Tick(inputs, events) = replay.step(7, u64::MAX).unwrap() else {
            panic!("expected replay to align its first recorded event")
        };
        assert_eq!(inputs[0].unwrap().held, bits::A);
        assert!(events.contains(&ShimEvent::ButtonDown(0)));
        let ReplayStep::Tick(_, events) = replay.step(8, u64::MAX).unwrap() else {
            panic!("expected the next recorded tick")
        };
        assert!(events.contains(&ShimEvent::ButtonUp(0)));
        assert!(matches!(replay.step(9, u64::MAX).unwrap(), ReplayStep::End));
    }

    #[test]
    fn replay_hold_releases_input_and_keeps_simulation_running_after_route_end() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.hold_after_end = true;
        assert!(matches!(
            replay.step(1, u64::MAX).unwrap(),
            ReplayStep::Tick(_, _)
        ));

        let ReplayStep::Tick(inputs, events) = replay.step(2, u64::MAX).unwrap() else {
            panic!("expected neutral tick while holding after the route")
        };
        assert!(inputs.iter().all(Option::is_none));
        assert!(events.contains(&ShimEvent::ButtonUp(0)));

        let ReplayStep::Tick(inputs, events) = replay.step(3, u64::MAX).unwrap() else {
            panic!("expected another neutral tick")
        };
        assert!(inputs.iter().all(Option::is_none));
        assert!(events.is_empty());
    }

    #[test]
    fn replay_hold_preserves_late_recorded_ticks_in_order() {
        let mut replay = playback([Some(input(bits::A)), None, None, None]);
        replay.ticks.push_back(Tick {
            sim_tick: 2,
            host_poll: 8,
            wall_ns: 50,
            inputs: [None; 4],
        });
        replay.end_tick = 2;
        replay.hold_after_end = true;

        assert!(matches!(
            replay.step(1, u64::MAX).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        let ReplayStep::Tick(_, events) = replay.step(3, u64::MAX).unwrap() else {
            panic!("expected a late recorded tick to be kept")
        };
        assert!(events.contains(&ShimEvent::ButtonUp(0)));
    }

    #[test]
    fn replay_ui_returns_same_tick_events_in_order_and_preserves_late_events() {
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ui = VecDeque::from([
            Ui {
                // Semantic actions are due by host poll even when the guest
                // controller callback is observing a different sim tick.
                sim_tick: 99,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page: 0x2d0be0,
                event: RecordedUiEvent::Focus(7),
            },
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page: 0x2d0be0,
                event: RecordedUiEvent::Activate(7),
            },
        ]);
        assert!(replay.ui_events(3, 0, Some(0x2d0be0)).unwrap().is_empty());
        assert_eq!(replay.ui_events(4, 0, Some(0x2d0be0)).unwrap().len(), 2);
        replay.ui.push_back(Ui {
            sim_tick: 2,
            host_poll: 8,
            wall_ns: 50,
            context: 0,
            player: 0,
            page: 0x2d0be0,
            event: RecordedUiEvent::Back,
        });
        replay.ticks.clear();
        assert!(replay.ui_events(7, 0, Some(0x2d0be0)).unwrap().is_empty());
        assert_eq!(replay.ui_events(8, 0, Some(0x2d0be0)).unwrap().len(), 1);
    }

    #[test]
    fn guest_owned_poll_routes_semantic_back_and_scroll_rows_in_order() {
        let mut replay = playback([None; 4]);
        replay.ui = VecDeque::from([
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page: 0xDEAD_0100,
                event: RecordedUiEvent::Back,
            },
            Ui {
                sim_tick: 100,
                host_poll: 8,
                wall_ns: 21,
                context: 0,
                player: 0,
                page: 0xDEAD_0200,
                event: RecordedUiEvent::Scroll(0, 1),
            },
        ]);

        let ReplayStep::Tick(_, events) = replay
            .step_at_host_poll_with_owners(0, 4, 0, true, None, Some(0x313010))
            .unwrap()
        else {
            panic!("expected the guest-owned recorded poll")
        };
        assert_eq!(events, [ShimEvent::ButtonDown(1)]);

        replay.ticks.push_back(Tick {
            sim_tick: 2,
            host_poll: 8,
            wall_ns: 50,
            inputs: [None; 4],
        });
        let ReplayStep::Tick(_, next) = replay
            .step_at_host_poll_with_owners(0, 8, 1, true, None, Some(0x313010))
            .unwrap()
        else {
            panic!("expected the next guest-owned action poll")
        };
        assert_eq!(next, [ShimEvent::ButtonUp(1), ShimEvent::ButtonDown(12)]);
        assert!(replay.ui.is_empty());

        replay.ticks.push_back(Tick {
            sim_tick: 3,
            host_poll: 12,
            wall_ns: 80,
            inputs: [None; 4],
        });
        let ReplayStep::Tick(_, releases) = replay
            .step_at_host_poll_with_owners(0, 12, 2, true, None, Some(0x313010))
            .unwrap()
        else {
            panic!("expected the guest-owned release poll")
        };
        assert_eq!(releases, [ShimEvent::ButtonUp(12)]);
    }

    #[test]
    fn guest_owned_scroll_routes_vertical_and_horizontal_axes() {
        assert_eq!(guest_ui_button(&RecordedUiEvent::Start), Some(6));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Activate(0)), Some(0));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Scroll(0, 1)), Some(12));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Scroll(0, -1)), Some(11));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Scroll(1, 1)), Some(14));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Scroll(1, -1)), Some(13));
        // T3723: list-selection Scroll on any list id steps the list; Back is delivered on list pages.
        assert_eq!(
            guest_ui_button(&RecordedUiEvent::Scroll(0x313010, 1)),
            Some(12)
        );
        assert_eq!(
            guest_ui_button(&RecordedUiEvent::Scroll(0x2CA960, -1)),
            Some(11)
        );
        assert_eq!(guest_ui_button(&RecordedUiEvent::Back), Some(1));
        assert_eq!(guest_ui_button(&RecordedUiEvent::Scroll(7, 0)), None);
    }

    #[test]
    fn registered_page_row_waits_until_its_owner_is_live() {
        let mut replay = playback([None; 4]);
        replay.ui = VecDeque::from([Ui {
            sim_tick: 1,
            host_poll: 4,
            wall_ns: 20,
            context: 0,
            player: 0,
            page: 0x313010,
            event: RecordedUiEvent::Back,
        }]);

        let ui_events = replay.ui_events_for_pages(4, 0, None, None).unwrap();
        assert!(ui_events.is_empty());
        assert_eq!(replay.ui.len(), 1);

        let ReplayStep::Tick(_, events) = replay
            .step_at_host_poll_with_owners(0, 4, 0, true, None, None)
            .unwrap()
        else {
            panic!("expected the guest-owned tick while the page row remains pending")
        };

        assert!(events.is_empty());
        assert_eq!(replay.ui.len(), 1);
        assert!(native_ui_page_has_handler(0x313010));
        assert!(!native_ui_page_owns_poll(0x313010, None));
        assert!(native_ui_page_owns_poll(0x313010, Some(0x313010)));
        assert_eq!(
            guest_ui_button_for_page(0x313010, &RecordedUiEvent::Back, None),
            None
        );
        assert_eq!(
            guest_ui_button_for_page(0x313010, &RecordedUiEvent::Back, Some(0x31AA10)),
            Some(1)
        );
        assert_eq!(
            guest_ui_button_for_page(0x313010, &RecordedUiEvent::Back, Some(0x313010)),
            Some(1)
        );
        assert!(!native_ui_page_has_handler(0xDEAD_0100));
    }

    #[test]
    fn registered_native_page_row_stays_pending_during_a_transition_gap() {
        let mut replay = playback([None; 4]);
        replay.ui = VecDeque::from([Ui {
            sim_tick: 1,
            host_poll: 4,
            wall_ns: 20,
            context: 0,
            player: 0,
            page: 0x3133E0,
            event: RecordedUiEvent::Back,
        }]);

        assert!(native_ui_page_has_handler(0x3133E0));
        let events = replay.ui_events_for_pages(4, 0, None, None).unwrap();
        assert!(events.is_empty());
        assert_eq!(replay.ui.len(), 1);

        let ReplayStep::Tick(_, guest_events) = replay
            .step_at_host_poll_with_owners(0, 4, 0, true, None, None)
            .unwrap()
        else {
            panic!("expected the guest-owned transition-gap poll")
        };
        assert!(guest_events.is_empty());
        assert_eq!(replay.ui.len(), 1);

        let owner_events = replay
            .ui_events_for_pages(5, 0, Some(0x3133E0), None)
            .unwrap();
        assert_eq!(owner_events.len(), 1);
        assert_eq!(owner_events[0].page, 0x3133E0);
        assert_eq!(owner_events[0].event, RecordedUiEvent::Back);
        assert!(replay.ui.is_empty());
    }

    #[test]
    fn guest_poll_keeps_ui_rows_until_a_guest_page_owns_input() {
        let mut replay = playback([None; 4]);
        replay.ui = VecDeque::from([Ui {
            sim_tick: 1,
            host_poll: 4,
            wall_ns: 20,
            context: 0,
            player: 0,
            page: 0xDEAD_0100,
            event: RecordedUiEvent::Back,
        }]);

        let ReplayStep::Tick(_, events) = replay
            .step_at_host_poll_with_owners(0, 4, 0, true, None, None)
            .unwrap()
        else {
            panic!("expected the guest-owned recorded poll")
        };

        assert!(events.is_empty());
        assert_eq!(replay.ui.len(), 1);

        let (row, button) =
            take_guest_owned_ui_event(&mut replay.ui, Some(4), None, Some(0x3179D0))
                .expect("the live guest page owns this due row despite its stale recorded builder");
        assert_eq!(row.page, 0xDEAD_0100);
        assert_eq!(button, 1);
        assert!(replay.ui.is_empty());
    }

    #[test]
    fn replay_dispatches_native_ui_on_recorded_frame_poll_before_first_fixed_tick() {
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ui = VecDeque::from([Ui {
            sim_tick: 485,
            host_poll: 2_147,
            wall_ns: 15_463_413_248,
            context: 0,
            player: 0,
            page: 0x2cac30,
            event: RecordedUiEvent::Start,
        }]);

        // A UI event recorded at frame poll 2,147 remains pending at 900 and
        // dispatches when replay reaches that frame, independent of wall time.
        assert!(
            replay
                .ui_events_for_pages(900, u64::MAX, Some(0x2cac30), None)
                .unwrap()
                .is_empty(),
            "recorded host frame must gate the event"
        );
        let events = replay
            .ui_events_for_pages(2_147, 0, Some(0x2cac30), None)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sim_tick, 485);
        assert_eq!(events[0].event, RecordedUiEvent::Start);
    }

    #[test]
    fn replay_routes_same_poll_rows_to_current_owner_in_recorded_order() {
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ui = VecDeque::from([
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page: 0x2d0be0,
                event: RecordedUiEvent::Activate(7),
            },
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 22,
                context: 0,
                player: 0,
                page: 0x2d2350,
                event: RecordedUiEvent::Start,
            },
        ]);

        let routed = replay.ui_events(4, u64::MAX, Some(0x2d0be0)).unwrap();
        assert_eq!(routed.len(), 2);
        assert!(routed.iter().all(|row| row.page == 0x2d0be0));
        assert_eq!(routed[0].event, RecordedUiEvent::Activate(7));
        assert_eq!(routed[1].event, RecordedUiEvent::Start);
        assert!(replay.ui.is_empty());
    }

    #[test]
    fn replay_page_mismatch_routes_semantic_row_to_current_owner_without_stalling_ticks() {
        let mut replay = playback([Some(input(0)), Some(input(0)), Some(input(0)), None]);
        replay.end_tick = 3;
        replay.ticks.extend([
            Tick {
                sim_tick: 2,
                host_poll: 8,
                wall_ns: 20,
                inputs: [Some(input(0)), None, None, None],
            },
            Tick {
                sim_tick: 3,
                host_poll: 12,
                wall_ns: 30,
                inputs: [Some(input(0)), None, None, None],
            },
        ]);
        replay.ui.push_back(Ui {
            sim_tick: 2,
            host_poll: 8,
            wall_ns: 20,
            context: 0,
            player: 0,
            page: 0x2d2350,
            event: RecordedUiEvent::Start,
        });

        assert!(matches!(
            replay.step(1, u64::MAX).unwrap(),
            ReplayStep::Tick(_, _)
        ));
        let routed = replay.ui_events(8, u64::MAX, Some(0x2d0be0)).unwrap();
        assert_eq!(routed.len(), 1, "the current Rust page owns this poll");
        assert_eq!(routed[0].page, 0x2d0be0);
        assert_eq!(routed[0].event, RecordedUiEvent::Start);
        assert!(replay.ui.is_empty());
        assert!(matches!(
            replay.step(2, u64::MAX).unwrap(),
            ReplayStep::Tick(_, _)
        ));

        assert!(
            matches!(replay.step(3, u64::MAX).unwrap(), ReplayStep::Tick(_, _)),
            "a delivered UI action must not pin later gameplay ticks"
        );
        assert!(replay.ui.is_empty());
    }

    #[test]
    fn replay_routes_due_rows_to_the_current_page_without_skipping_earlier_rows() {
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ticks.clear();
        replay.timeline_anchor = Some((1, 1));
        replay.ui = VecDeque::from([
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page: 0x2d0be0,
                event: RecordedUiEvent::Activate(7),
            },
            Ui {
                sim_tick: 2,
                host_poll: 8,
                wall_ns: 30,
                context: 0,
                player: 0,
                page: 0x194980,
                event: RecordedUiEvent::Scroll(0, 1),
            },
            Ui {
                sim_tick: 3,
                host_poll: 12,
                wall_ns: 40,
                context: 0,
                player: 0,
                page: 0x2d0be0,
                event: RecordedUiEvent::Back,
            },
            Ui {
                sim_tick: 3,
                host_poll: 12,
                wall_ns: 42,
                context: 0,
                player: 0,
                page: 0x194980,
                event: RecordedUiEvent::Start,
            },
        ]);

        let delivered = replay.ui_events(4, 0, Some(0x194980)).unwrap();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].page, 0x194980);
        assert_eq!(delivered[0].event, RecordedUiEvent::Activate(7));
        let delivered = replay.ui_events(8, 10, Some(0x194980)).unwrap();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].page, 0x194980);
        assert_eq!(delivered[0].event, RecordedUiEvent::Scroll(0, 1));
        let delivered = replay.ui_events(12, 22, Some(0x194980)).unwrap();
        assert_eq!(delivered.len(), 2);
        assert_eq!(delivered[0].event, RecordedUiEvent::Back);
        assert_eq!(delivered[1].event, RecordedUiEvent::Start);
        assert!(delivered.iter().all(|row| row.page == 0x194980));
        assert!(replay.ui.is_empty());
    }

    #[test]
    fn replay_holds_profile_text_events_until_the_text_handler_owns_the_shared_builder() {
        let page = 0x194980;
        let mut replay = playback([Some(input(0)), None, None, None]);
        replay.ui = VecDeque::from([
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 20,
                context: 0,
                player: 0,
                page,
                event: RecordedUiEvent::TextInput('P'),
            },
            Ui {
                sim_tick: 1,
                host_poll: 4,
                wall_ns: 21,
                context: 0,
                player: 0,
                page,
                event: RecordedUiEvent::TextCommit {
                    widget: 0,
                    text: b"Player".to_vec(),
                },
            },
        ]);

        let selection_page = replay
            .ui_events_for_pages(4, u64::MAX, Some(page), None)
            .unwrap();
        assert!(selection_page.is_empty());
        assert_eq!(replay.ui.len(), 2);

        let text_page = replay
            .ui_events_for_pages(4, u64::MAX, Some(page), Some(page))
            .unwrap();
        assert_eq!(text_page.len(), 2);
        assert!(replay.ui.is_empty());
    }
}

#[cfg(test)]
mod tick_clock_tests {
    use super::{recorded_poll_at, tick_clock_switch};

    #[test]
    fn tick_clock_maps_a_tick_to_its_recorded_poll() {
        let table = [(1, 9), (2, 12), (5, 30)];
        assert_eq!(recorded_poll_at(&table, 0), 0);
        assert_eq!(recorded_poll_at(&table, 1), 9);
        assert_eq!(recorded_poll_at(&table, 4), 12);
        assert_eq!(recorded_poll_at(&table, 99), 124);
        assert!(tick_clock_switch(Some("tick")));
        assert!(!tick_clock_switch(Some("poll")));
        assert!(!tick_clock_switch(None));
    }
}

#[cfg(test)]
mod t3453_tests {
    use super::{accept_edge_near, recorded_poll_at, take_guest_bound};
    use std::collections::VecDeque;
    use tsfp_input::shim::ShimEvent;

    /// T3453: the Map Maker legacy pointer shim matches recorded A edges (recorded host polls) on the
    /// replay clock. The ticks and polls are the measured mapmaker-editor-20261018 values: the Select
    /// Mode popup is up from tick 825, the recorded click is the A edge at poll 2677 (tick 952), and
    /// the boot-time A edge at poll 1028 belongs to tick 104.
    #[test]
    fn legacy_shim_matches_recorded_edges_on_the_replay_clock() {
        let table = [
            (104, 1028),
            (826, 2452),
            (950, 2674),
            (951, 2675),
            (952, 2677),
        ];
        let edges = [1028, 2677];
        assert!(!accept_edge_near(
            &edges,
            recorded_poll_at(&table, 826),
            3,
            1
        ));
        assert!(accept_edge_near(
            &edges,
            recorded_poll_at(&table, 950),
            3,
            1
        ));
        assert!(accept_edge_near(
            &edges,
            recorded_poll_at(&table, 952),
            3,
            1
        ));
        // The live window poll count of the fast-forward replay stood at 1026 by tick 826 and
        // matched the boot edge: the misfire that opened the editor at a load-dependent tick.
        assert!(accept_edge_near(&edges, 1026, 3, 1));
    }

    /// T3453: queued replay events are routed as one batch at the guest's poll: all of them reach
    /// the input queue, or none when a Rust page owns input at that poll, and the queue empties.
    #[test]
    fn guest_bound_events_are_routed_by_the_page_state_at_the_guest_poll() {
        let events = [
            ShimEvent::Axis {
                axis: 2,
                value: 16676,
            },
            ShimEvent::Axis {
                axis: 3,
                value: -13730,
            },
            ShimEvent::ButtonDown(0),
        ];
        let mut queue: VecDeque<ShimEvent> = events.iter().copied().collect();
        assert_eq!(take_guest_bound(&mut queue, false), events.to_vec());
        assert!(queue.is_empty());
        queue.extend(events);
        assert!(take_guest_bound(&mut queue, true).is_empty());
        assert!(
            queue.is_empty(),
            "rows a Rust page owned are consumed, not replayed later"
        );
        assert!(take_guest_bound(&mut queue, false).is_empty());
    }
}
