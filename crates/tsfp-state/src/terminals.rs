// SPDX-License-Identifier: GPL-3.0-or-later
//! T2378 readable, Rust-owned mission-terminal state.
//!
//! The recovered contracts and limits are in `docs/t1665-rea-terminal*.md`. This module is
//! deliberately opt-in: it does not alter the existing translated runtime selector. Callers own
//! stable IDs and pass input/effects explicitly, so no guest-memory offsets or synthetic pad
//! presses cross this boundary.
use alloc::vec::Vec;

/// Stable within one `TerminalState`; child-list positions are not widget identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct WidgetId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScreenId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalWidget {
    pub id: WidgetId,
    /// Signed ordering value. It is not an identity or a child-list index.
    pub rank: i32,
    /// Bit 1 in the original guest flags means interactive/active for focus selection.
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalScreen {
    pub id: ScreenId,
    /// Global widget identities in original child-list order.
    pub children: Vec<WidgetId>,
    pub pointer: PointerPosition,
    /// Converted parent timer from original field +0x50.
    pub timer: i32,
    pub previous_held: u16,
    pub script: ScriptState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalState {
    pub widgets: Vec<TerminalWidget>,
    pub screens: Vec<TerminalScreen>,
    pub focus: Option<WidgetId>,
    pub current_parent: Option<ScreenId>,
    pub current_widget: Option<WidgetId>,
    pub current_timer: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalError {
    MissingScreen(ScreenId),
    MissingWidget(WidgetId),
    InvalidChildPosition(usize),
}

impl TerminalState {
    pub fn new(screens: Vec<TerminalScreen>, widgets: Vec<TerminalWidget>) -> Self {
        Self {
            screens,
            widgets,
            focus: None,
            current_parent: None,
            current_widget: None,
            current_timer: 0,
        }
    }

    fn screen(&self, id: ScreenId) -> Result<&TerminalScreen, TerminalError> {
        self.screens
            .iter()
            .find(|screen| screen.id == id)
            .ok_or(TerminalError::MissingScreen(id))
    }

    fn screen_mut(&mut self, id: ScreenId) -> Result<&mut TerminalScreen, TerminalError> {
        self.screens
            .iter_mut()
            .find(|screen| screen.id == id)
            .ok_or(TerminalError::MissingScreen(id))
    }

    fn widget(&self, id: WidgetId) -> Result<&TerminalWidget, TerminalError> {
        self.widgets
            .iter()
            .find(|widget| widget.id == id)
            .ok_or(TerminalError::MissingWidget(id))
    }

    fn widget_mut(&mut self, id: WidgetId) -> Result<&mut TerminalWidget, TerminalError> {
        self.widgets
            .iter_mut()
            .find(|widget| widget.id == id)
            .ok_or(TerminalError::MissingWidget(id))
    }

    /// Rank mutation from 0x36E3A0. The focus pointer is published after all rank writes.
    pub fn bring_to_front(&mut self, selected: WidgetId) -> Result<(), TerminalError> {
        let old_rank = self.widget(selected)?.rank;
        for widget in &mut self.widgets {
            if widget.rank > old_rank {
                widget.rank -= 1;
            }
        }
        self.widget_mut(selected)?.rank = self.widgets.len() as i32 - 1;
        self.focus = Some(selected);
        Ok(())
    }

    /// Pick the first child with the greatest rank strictly above -1 and active bit 2 set.
    /// No candidate leaves the previous focus and every rank unchanged.
    pub fn focus_topmost(&mut self, screen: ScreenId) -> Result<Option<WidgetId>, TerminalError> {
        let children = self.screen(screen)?.children.clone();
        let mut best_rank = -1;
        let mut best = None;
        for id in children {
            let widget = self.widget(id)?;
            if widget.rank > best_rank && widget.flags & 2 != 0 {
                best_rank = widget.rank;
                best = Some(id);
            }
        }
        if let Some(id) = best {
            self.bring_to_front(id)?;
        }
        Ok(best)
    }

    /// Activation preserves the original's reorder/timer effects even when it reports already
    /// active. `timer` is the caller's signed-int-to-float timing result converted by truncation.
    pub fn activate_child(
        &mut self,
        screen: ScreenId,
        child_position: usize,
        timer: i32,
    ) -> Result<bool, TerminalError> {
        let selected = *self
            .screen(screen)?
            .children
            .get(child_position)
            .ok_or(TerminalError::InvalidChildPosition(child_position))?;
        let already_active = self.widget(selected)?.flags & 2 != 0;
        self.screen_mut(screen)?.timer = timer;
        self.widget_mut(selected)?.flags |= 2;
        self.bring_to_front(selected)?;
        if already_active {
            return Ok(false);
        }
        self.current_parent = Some(screen);
        self.current_widget = Some(selected);
        self.current_timer = timer;
        Ok(true)
    }
}

/// A physical sample is already normalized from the input record's left-stick axes 1 and 2.
/// Keyboard and mouse adapters can produce this value directly without emulating a gamepad.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicalTerminalInput {
    pub held_after_consumption: u32,
    pub activation_mask: u32,
    pub left_x: f32,
    pub left_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalPadRecord {
    pub held: u32,
    pub consumed: u32,
    pub dead_zone: f32,
    /// Game input record axes 0, 1 and 2; terminals consume left X/Y at 1 and 2.
    pub axes: [i32; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalPadBank {
    /// The original 0x6B9888 helper gate; a set gate yields zero held/axis values.
    pub blocked: bool,
    /// Zero selects the remapped physical records. Every nonzero value selects effective records.
    pub selector: u32,
    pub physical_port_by_slot: Vec<usize>,
    pub physical_records: Vec<TerminalPadRecord>,
    pub effective_records: Vec<TerminalPadRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicalInputError {
    SlotOutOfRange(usize),
    PortOutOfRange(usize),
}

/// Select the input record independently for each sample, apply consumed-button masking, and
/// normalize only the terminal's left-stick axes. Invalid typed indices are explicit Rust errors;
/// the original helpers did not guard them.
pub fn sample_terminal_pad(
    bank: &TerminalPadBank,
    slot: usize,
    activation_mask: u32,
) -> Result<PhysicalTerminalInput, PhysicalInputError> {
    if bank.blocked {
        return Ok(PhysicalTerminalInput {
            held_after_consumption: 0,
            activation_mask,
            left_x: 0.0,
            left_y: 0.0,
        });
    }
    let record = if bank.selector == 0 {
        let port = *bank
            .physical_port_by_slot
            .get(slot)
            .ok_or(PhysicalInputError::SlotOutOfRange(slot))?;
        bank.physical_records
            .get(port)
            .ok_or(PhysicalInputError::PortOutOfRange(port))?
    } else {
        bank.effective_records
            .get(slot)
            .ok_or(PhysicalInputError::SlotOutOfRange(slot))?
    };
    Ok(PhysicalTerminalInput {
        held_after_consumption: record.held & !record.consumed,
        activation_mask,
        left_x: normalize_axis(record.axes[1], record.dead_zone),
        left_y: normalize_axis(record.axes[2], record.dead_zone),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    Physical,
    Script,
}

/// Numeric phase derived from current input and the stored low 16 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    Idle,
    Release,
    Held,
    Press,
}

pub fn terminal_event(held: bool, previous_held: u16) -> TerminalEvent {
    match (held, previous_held != 0) {
        (false, false) => TerminalEvent::Idle,
        (false, true) => TerminalEvent::Release,
        (true, true) => TerminalEvent::Held,
        (true, false) => TerminalEvent::Press,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalInputFrame {
    pub held: bool,
    pub cursor_delta: [f32; 2],
    pub source: InputSource,
}

/// Script input wins for the current tick; a terminating script still owns that tick. Physical
/// held matching uses equality with the entire unconsumed mask, as in 0x3700D0.
pub fn resolve_terminal_input(
    physical: PhysicalTerminalInput,
    suppression_latch: bool,
    script: Option<ScriptOverride>,
) -> TerminalInputFrame {
    if let Some(override_input) = script {
        return TerminalInputFrame {
            held: override_input.held,
            cursor_delta: override_input.cursor_delta,
            source: InputSource::Script,
        };
    }
    TerminalInputFrame {
        held: !suppression_latch && physical.held_after_consumption == physical.activation_mask,
        cursor_delta: [physical.left_x, -physical.left_y],
        source: InputSource::Physical,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptOverride {
    pub held: bool,
    pub cursor_delta: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScriptRecord {
    Wait { duration: f32 },
    MoveTo { x: f32, y: f32 },
    Hold { duration: f32 },
    End,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptState {
    pub records: Vec<ScriptRecord>,
    pub cursor: usize,
    pub elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptTick {
    /// `None` means no cursor was active on entry and physical input may run.
    pub override_input: Option<ScriptOverride>,
    /// True only when a move record reached its completion condition this tick.
    pub move_completed: bool,
}

impl ScriptState {
    pub fn active(&self) -> bool {
        self.cursor < self.records.len()
    }

    /// Execute one script record at most. Timing inputs map to source globals 0x4E794C and
    /// 0x4E8CEC; constants 100, 0.2, 0.0001 and 2.0 are the measured original data values.
    /// This preserves the source's asymmetric zero/tiny-distance normalization.
    pub fn tick(&mut self, pointer: &mut PointerPosition, dt: f32, rate: i32) -> ScriptTick {
        if !self.active() {
            return ScriptTick {
                override_input: None,
                move_completed: false,
            };
        }
        self.elapsed += dt / rate as f32;
        let record = self.records[self.cursor];
        match record {
            ScriptRecord::Wait { duration } => {
                if self.elapsed < duration || self.elapsed.is_nan() || duration.is_nan() {
                    ScriptTick {
                        override_input: Some(ScriptOverride {
                            held: false,
                            cursor_delta: [0.0; 2],
                        }),
                        move_completed: false,
                    }
                } else {
                    self.advance()
                }
            }
            ScriptRecord::Hold { duration } => {
                let done = !(self.elapsed < duration || self.elapsed.is_nan() || duration.is_nan());
                let tick = if done {
                    self.advance()
                } else {
                    ScriptTick {
                        override_input: Some(ScriptOverride {
                            held: true,
                            cursor_delta: [0.0; 2],
                        }),
                        move_completed: false,
                    }
                };
                // The expiry tick is also held, even as the cursor advances.
                if done {
                    ScriptTick {
                        override_input: Some(ScriptOverride {
                            held: true,
                            cursor_delta: [0.0; 2],
                        }),
                        move_completed: false,
                    }
                } else {
                    tick
                }
            }
            ScriptRecord::MoveTo { x, y } => {
                let dx = x - pointer.x;
                let dy = y - pointer.y;
                let distance = sqrt32(dx * dx + dy * dy);
                let normal_x = if distance > 0.0001 { dx / distance } else { dx };
                let normal_y = dy / distance;
                let candidate = (self.elapsed / rate as f32) * dt * 100.0;
                let cap = distance * 0.2;
                let step = if cap > candidate { candidate } else { cap };
                let old = *pointer;
                pointer.x += step * normal_x;
                pointer.y += step * normal_y;
                let complete = (x - pointer.x).abs() < 2.0 && (y - pointer.y).abs() < 2.0;
                let cursor_delta = [pointer.x - old.x, pointer.y - old.y];
                if complete {
                    let _ = self.advance();
                    ScriptTick {
                        override_input: Some(ScriptOverride {
                            held: false,
                            cursor_delta,
                        }),
                        move_completed: true,
                    }
                } else {
                    ScriptTick {
                        override_input: Some(ScriptOverride {
                            held: false,
                            cursor_delta,
                        }),
                        move_completed: false,
                    }
                }
            }
            ScriptRecord::End => {
                self.cursor = self.records.len();
                // The live script owns the termination tick, and elapsed is intentionally kept.
                ScriptTick {
                    override_input: Some(ScriptOverride {
                        held: false,
                        cursor_delta: [0.0; 2],
                    }),
                    move_completed: false,
                }
            }
        }
    }

    fn advance(&mut self) -> ScriptTick {
        self.cursor += 1;
        self.elapsed = 0.0;
        ScriptTick {
            override_input: Some(ScriptOverride {
                held: false,
                cursor_delta: [0.0; 2],
            }),
            move_completed: false,
        }
    }
}

/// `no_std` binary32 square root for the finite, nonnegative script-distance domain. Eight
/// Newton steps from an exponent-scaled seed converge to the correctly rounded neighbourhood for
/// ordinary screen-coordinate magnitudes; NaN, infinity and invalid timing remain outside the
/// source-derived finite-input contract.
fn sqrt32(value: f32) -> f32 {
    if value == 0.0 || value.is_infinite() || value.is_nan() || value < 0.0 {
        return if value < 0.0 { f32::NAN } else { value };
    }
    let mut estimate = f32::from_bits((value.to_bits() >> 1) + 0x1fc0_0000);
    for _ in 0..8 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

/// Convert the physical axis record's signed integer values to left-stick coordinates with the
/// original asymmetric 128/127 scale and inclusive dead-zone. No clamping is added.
pub fn normalize_axis(raw: i32, dead_zone: f32) -> f32 {
    let offset = raw.wrapping_sub(128);
    let axis = if raw < 128 {
        offset as f32 * (1.0 / 128.0)
    } else {
        offset as f32 * (1.0 / 127.0)
    };
    if dead_zone <= 0.0 || dead_zone.is_nan() {
        return axis;
    }
    if axis >= -dead_zone && axis <= dead_zone {
        0.0
    } else if axis < -dead_zone {
        (axis + dead_zone) / (1.0 - dead_zone)
    } else {
        (axis - dead_zone) / (1.0 - dead_zone)
    }
}
