// SPDX-License-Identifier: GPL-3.0-or-later
//! Device layer and the per slot router (docs/t-mouse-keyboard-design.md 6.4, docs/t-contract-input.md 1 and 2).
//!
//! * Devices: gamepads (f32 analog, hotplug), one raw relative mouse (f64 accumulator), the keyboard (physical key names), the wheel.
//! * Slots 0..3 each have an optional pad and, for at most one slot (the single mouse owner), the mouse and keyboard.
//!   A slot with both follows the last device that was used, with a hysteresis of [`RouterConfig::hysteresis_ticks`] ticks.
//! * A slot on a pad forwards the pad transitions in order and unchanged (the exact stream the shell produced before the router,
//!   so a pad user stays trace equal). A slot on the mouse and keyboard turns the held keys into a virtual pad every tick.
//! * Every tick yields one [`PlayerInput`] per active slot.
use crate::accum::{LookAccumulator, WheelAccumulator};
use crate::bindings::{Action, Bindings};
use crate::player::{
    AssistMask, ButtonBits, Look, MenuIntent, PlayerInput, RumbleEffect, SourceKind, Vec2, bits,
};
use crate::shim::{SDL_BUTTON_BITS, SdlReport, ShimEvent, diff, report_from_player, to_i16};
use std::collections::{BTreeMap, BTreeSet};

pub type PadId = u32;
pub const SLOTS: usize = 4;
/// Ticks a wheel notch holds its button, so a guest poll cannot miss it.
pub const WHEEL_PULSE_TICKS: u8 = 3;
/// Raw counts in one tick that count as using the mouse for the last device wins rule.
pub const MOTION_ACTIVITY_COUNTS: f64 = 8.0;
/// Stick or trigger deflection that counts as using a pad.
pub const PAD_ACTIVITY: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    Pad(PadId),
    MouseKeyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadButton {
    South,
    East,
    West,
    North,
    Select,
    Mode,
    Start,
    LeftThumb,
    RightThumb,
    LeftBumper,
    RightBumper,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl PadButton {
    /// SDL3 gamepad button index (shell convention, events.rs).
    pub fn sdl_index(self) -> u8 {
        self as u8
    }

    fn binding_name(self) -> &'static str {
        match self {
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
            Self::North => "north",
            Self::Select => "select",
            Self::Mode => "mode",
            Self::Start => "start",
            Self::LeftThumb => "left_thumb",
            Self::RightThumb => "right_thumb",
            Self::LeftBumper => "left_shoulder",
            Self::RightBumper => "right_shoulder",
            Self::DPadUp => "dpad_up",
            Self::DPadDown => "dpad_down",
            Self::DPadLeft => "dpad_left",
            Self::DPadRight => "dpad_right",
        }
    }
}

fn pad_button_from_index(index: usize) -> Option<PadButton> {
    Some(match index {
        0 => PadButton::South,
        1 => PadButton::East,
        2 => PadButton::West,
        3 => PadButton::North,
        4 => PadButton::Select,
        5 => PadButton::Mode,
        6 => PadButton::Start,
        7 => PadButton::LeftThumb,
        8 => PadButton::RightThumb,
        9 => PadButton::LeftBumper,
        10 => PadButton::RightBumper,
        11 => PadButton::DPadUp,
        12 => PadButton::DPadDown,
        13 => PadButton::DPadLeft,
        14 => PadButton::DPadRight,
        _ => return None,
    })
}

fn action_button_bit(action: Action) -> ButtonBits {
    match action {
        Action::A => bits::A,
        Action::B => bits::B,
        Action::X => bits::X,
        Action::Y => bits::Y,
        Action::Start => bits::START,
        Action::Back => bits::BACK,
        Action::L3 => bits::L3,
        Action::R3 => bits::R3,
        Action::Black => bits::BLACK,
        Action::White => bits::WHITE,
        Action::DpadUp => bits::DPAD_UP,
        Action::DpadDown => bits::DPAD_DOWN,
        Action::DpadLeft => bits::DPAD_LEFT,
        Action::DpadRight => bits::DPAD_RIGHT,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButtonId {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButtonId {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Middle => "Middle",
            Self::Back => "Back",
            Self::Forward => "Forward",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    PadConnected(PadId),
    PadDisconnected(PadId),
    /// `value` 0..1 (a button is down above 0.5).
    PadButton {
        pad: PadId,
        button: PadButton,
        value: f32,
    },
    /// Sticks -1..1 with up and right positive, triggers 0..1.
    PadAxis {
        pad: PadId,
        axis: PadAxis,
        value: f32,
    },
    /// Raw relative counts, down and right positive.
    MouseMotion {
        dx: f64,
        dy: f64,
    },
    MouseButton {
        button: MouseButtonId,
        down: bool,
    },
    /// Notches, up positive.
    Wheel {
        notches: f64,
    },
    /// Physical key name (Bevy `KeyCode` Debug name). Repeats must be dropped by the caller.
    Key {
        name: String,
        down: bool,
    },
    FocusLost,
}

#[derive(Clone, Debug)]
pub struct RouterConfig {
    pub bindings: Bindings,
    /// The slot that owns the mouse and keyboard (`None` = no mouse and keyboard player).
    pub mouse_keyboard_slot: Option<u8>,
    pub hysteresis_ticks: u64,
    /// T2161: the mouse look reaches the guest through the raw look mailbox (degrees in `PlayerInput.look`), so the virtual
    /// right stick of the interim path is held at zero for a mouse and keyboard slot.
    pub raw_look: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            bindings: Bindings::default(),
            mouse_keyboard_slot: None,
            hysteresis_ticks: 30,
            raw_look: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PadDev {
    buttons: u16,
    axes: [f32; 6],
    axes_seen: [bool; 6],
}

impl PadDev {
    /// SDL convention report (Y negated) of the current state.
    fn report(&self) -> SdlReport {
        let mut axes = [None; 6];
        for (at, value) in self.axes.iter().enumerate() {
            if self.axes_seen[at] {
                axes[at] = Some(sdl_axis_value(at, *value));
            }
        }
        SdlReport {
            buttons: self.buttons,
            axes,
        }
    }
}

fn neutralize_missing_axes(sent: &SdlReport, target: &mut SdlReport) {
    for axis in 0..target.axes.len() {
        if target.axes[axis].is_none()
            && sent.axes[axis].is_some_and(|value| value != 0)
        {
            target.axes[axis] = Some(0);
        }
    }
}

fn sdl_axis_value(axis: usize, value: f32) -> i16 {
    match axis {
        1 | 3 => to_i16(-value),
        4 | 5 => to_i16(value.max(0.0)),
        _ => to_i16(value),
    }
}

fn axis_from_sdl(value: i16) -> f32 {
    // The SDL host path quantizes with `to_i16` (scale 32767), so invert that
    // mapping here. Dividing negative values by 32768 loses one count on the
    // return trip for every negative value other than i16::MIN.
    f32::from(value) / f32::from(i16::MAX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Active {
    Pad,
    Mk,
}

#[derive(Clone, Debug)]
struct Slot {
    pad: Option<PadId>,
    mk: bool,
    auto: bool,
    active: Option<Active>,
    last_switch: u64,
    /// What the guest queue has been told for this slot.
    sent: SdlReport,
    held_prev: ButtonBits,
    step: u64,
}

impl Slot {
    fn new() -> Self {
        Self {
            pad: None,
            mk: false,
            auto: false,
            active: None,
            last_switch: 0,
            sent: SdlReport::default(),
            held_prev: 0,
            step: 0,
        }
    }
}

pub struct Router {
    config: RouterConfig,
    player_bindings: Option<[Bindings; SLOTS]>,
    slots: [Slot; SLOTS],
    pads: BTreeMap<PadId, PadDev>,
    pad_activity: BTreeMap<PadId, u64>,
    keys_down: BTreeSet<String>,
    mouse_down: BTreeSet<&'static str>,
    wheel: WheelAccumulator,
    pulses: [u8; 2],
    /// Slot keys pressed since the last tick (weapon entry 1..=9), consumed by the mouse and keyboard owner.
    slot_keys: Vec<u8>,
    weapon_requests: [Option<u8>; SLOTS],
    /// T3723: slots whose player is mounted (turret, vehicle); raw counts do not reach them, so their look stays on the stick.
    mounted: u8,
    look: LookAccumulator,
    mk_activity: u64,
    tick: u64,
    out: Vec<(u8, ShimEvent)>,
    last: [Option<PlayerInput>; SLOTS],
    device_assignments: Option<[crate::settings::Device; SLOTS]>,
}

impl Router {
    pub fn new(config: RouterConfig) -> Self {
        let mut router = Self {
            device_assignments: None,
            player_bindings: None,
            slots: std::array::from_fn(|_| Slot::new()),
            pads: BTreeMap::new(),
            pad_activity: BTreeMap::new(),
            keys_down: BTreeSet::new(),
            mouse_down: BTreeSet::new(),
            wheel: WheelAccumulator::default(),
            pulses: [0; 2],
            slot_keys: Vec::new(),
            weapon_requests: [None; SLOTS],
            mounted: 0,
            look: LookAccumulator::default(),
            mk_activity: 0,
            tick: 0,
            out: Vec::new(),
            last: [None; SLOTS],
            config,
        };
        if let Some(slot) = router.config.mouse_keyboard_slot {
            router.set_mouse_keyboard(Some(slot));
        }
        router
    }

    /// T2200: switch the mouse look between the raw look mailbox (`true`, virtual right stick held at zero) and the interim
    /// virtual right stick of T2160 (`false`). The next tick sends the difference to the guest queue.
    pub fn set_raw_look(&mut self, on: bool) {
        self.config.raw_look = on;
    }

    /// T3723: slots (bit n) whose player is mounted or possessed; in raw look mode they keep the virtual right stick.
    pub fn set_mounted(&mut self, mask: u8) {
        self.mounted = mask;
    }

    /// Native host settings update; no input replay or guest-memory mutation.
    pub fn set_sensitivity(&mut self, sensitivity: crate::accum::Sensitivity) {
        self.config.bindings.sensitivity = sensitivity;
    }

    /// Update owned bindings without changing device ownership or hot-swap policy.
    pub fn set_bindings(&mut self, bindings: Bindings) {
        self.player_bindings = None;
        self.config.bindings = bindings;
    }

    /// Leaving a profile restores the launcher's automatic pad/M+K policy.
    /// Connected physical devices remain available, while old ownership and
    /// held keyboard/mouse input are released before allocating them afresh.
    pub fn restore_automatic_devices(&mut self) {
        if self.device_assignments.take().is_none() {
            return;
        }
        self.keys_down.clear();
        self.mouse_down.clear();
        self.look = LookAccumulator::default();
        self.weapon_requests = [None; SLOTS];
        self.slot_keys.clear();
        for index in 0..SLOTS {
            self.switch(index, None);
            self.slots[index].pad = None;
            self.slots[index].mk = false;
            self.slots[index].auto = false;
        }
        self.set_mouse_keyboard(self.config.mouse_keyboard_slot);
        let pads: Vec<_> = self.pads.keys().copied().take(SLOTS).collect();
        for (index, pad) in pads.into_iter().enumerate() {
            self.slots[index].pad = Some(pad);
            self.settle(index);
        }
    }

    pub fn set_player_bindings(&mut self, settings: &crate::settings::InputSettings) {
        self.config.bindings = settings.bindings.clone();
        self.player_bindings = Some(std::array::from_fn(|i| {
            settings.player_bindings(i).unwrap().clone()
        }));
    }
    fn bindings_for(&self, index: usize) -> &Bindings {
        self.player_bindings
            .as_ref()
            .map_or(&self.config.bindings, |maps| &maps[index])
    }
    fn mk_bindings(&self) -> &Bindings {
        let slot = self.slots.iter().position(|s| s.mk).unwrap_or(0);
        self.bindings_for(slot)
    }

    /// Apply native settings directly. Pad numbers identify sorted connected device IDs.
    /// Explicit assignments disable automatic pad allocation; disconnected choices stay reserved.
    pub fn apply_settings(&mut self, settings: &crate::settings::InputSettings) {
        self.set_player_bindings(settings);
        if self.device_assignments != Some(settings.players) {
            self.device_assignments = Some(settings.players);
            self.keys_down.clear();
            self.mouse_down.clear();
            self.look = LookAccumulator::default();
            self.weapon_requests = [None; SLOTS];
            self.slot_keys.clear();
        }
        self.assign_devices();
    }
    fn assign_devices(&mut self) {
        let Some(assignments) = self.device_assignments else {
            return;
        };
        let pads: Vec<_> = self.pads.keys().copied().collect();
        for (index, device) in assignments.into_iter().enumerate() {
            let pad = match device {
                crate::settings::Device::Pad(n) => pads.get(n as usize).copied(),
                _ => None,
            };
            let mk = device == crate::settings::Device::KeyboardMouse;
            if self.slots[index].pad != pad || self.slots[index].mk != mk {
                self.switch(index, None);
                self.slots[index].pad = pad;
                self.slots[index].mk = mk;
                self.slots[index].auto = false;
                self.settle(index);
            }
        }
    }

    pub fn raw_look(&self) -> bool {
        self.config.raw_look
    }

    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// The single mouse owner. Moving it demotes the previous owner to its pad (or no source).
    pub fn set_mouse_keyboard(&mut self, slot: Option<u8>) {
        // A request belongs to the owner at the time it was made, not to a later owner.
        self.weapon_requests = [None; SLOTS];
        self.slot_keys.clear();
        for index in 0..SLOTS {
            if self.slots[index].mk && Some(index as u8) != slot {
                self.slots[index].mk = false;
                self.slots[index].auto = false;
                self.settle(index);
            }
        }
        if let Some(slot) = slot.filter(|slot| usize::from(*slot) < SLOTS) {
            let tick = self.tick;
            let target = &mut self.slots[usize::from(slot)];
            target.mk = true;
            target.auto = true;
            target.last_switch = tick;
            self.settle(usize::from(slot));
        }
    }

    pub fn mouse_owner(&self) -> Option<u8> {
        self.slots
            .iter()
            .position(|slot| slot.mk)
            .map(|index| index as u8)
    }

    pub fn source(&self, slot: u8) -> Option<InputSource> {
        let slot = self.slots.get(usize::from(slot))?;
        match slot.active? {
            Active::Pad => slot.pad.map(InputSource::Pad),
            Active::Mk => Some(InputSource::MouseKeyboard),
        }
    }

    /// The mouse owner is currently on the mouse and keyboard (the shell grabs the pointer and Escape stays the pad START).
    pub fn mouse_keyboard_active(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.mk && slot.active == Some(Active::Mk))
    }

    pub fn last_input(&self, slot: u8) -> Option<PlayerInput> {
        self.last.get(usize::from(slot)).copied().flatten()
    }

    /// Pad of a slot that a rumble effect would go to: none while the slot is on the mouse and keyboard.
    pub fn route_rumble(
        &self,
        slot: u8,
        effect: RumbleEffect,
        mut sink: impl FnMut(PadId, RumbleEffect),
    ) {
        if let Some(slot) = self.slots.get(usize::from(slot)) {
            if let (Some(Active::Pad), Some(pad)) = (slot.active, slot.pad) {
                sink(pad, effect);
            }
        }
    }

    /// Ordered events for the guest queue since the last call, with the slot they belong to.
    pub fn drain_events(&mut self) -> Vec<(u8, ShimEvent)> {
        std::mem::take(&mut self.out)
    }

    fn target_report(&self, index: usize, slot: &Slot, active: Option<Active>) -> SdlReport {
        match (active, slot.pad) {
            (Some(Active::Pad), Some(pad)) => {
                let mut target = self.pad_report(index, pad);
                neutralize_missing_axes(&slot.sent, &mut target);
                target
            }
            _ => SdlReport {
                buttons: 0,
                axes: slot.sent.axes.map(|sent| sent.map(|_| 0)),
            },
        }
    }

    /// Recompute which device a slot is on after its devices changed and tell the guest the difference.
    fn settle(&mut self, index: usize) {
        let slot = &self.slots[index];
        let wanted = match (slot.pad.is_some(), slot.mk) {
            (true, true) => slot.active.or(Some(Active::Mk)),
            (true, false) => Some(Active::Pad),
            (false, true) => Some(Active::Mk),
            (false, false) => None,
        };
        if wanted != self.slots[index].active {
            self.switch(index, wanted);
        }
    }

    fn switch(&mut self, index: usize, active: Option<Active>) {
        let target = self.target_report(index, &self.slots[index], active);
        let tick = self.tick;
        let slot = &mut self.slots[index];
        let mut events = Vec::new();
        diff(&mut slot.sent, &target, &mut events);
        slot.active = active;
        slot.last_switch = tick;
        slot.held_prev = 0;
        self.out
            .extend(events.into_iter().map(|event| (index as u8, event)));
    }

    fn connect(&mut self, pad: PadId) {
        if self.pads.contains_key(&pad) {
            return;
        }
        self.pads.insert(pad, PadDev::default());
        if self.device_assignments.is_some() {
            self.assign_devices();
            return;
        }
        if let Some(index) = self.slots.iter().position(|slot| slot.pad.is_none()) {
            self.slots[index].pad = Some(pad);
            self.settle(index);
        }
    }

    fn disconnect(&mut self, pad: PadId) {
        if let Some(index) = self.slots.iter().position(|slot| slot.pad == Some(pad)) {
            if self.slots[index].active == Some(Active::Pad) {
                self.switch(index, None);
            }
            self.slots[index].pad = None;
            self.slots[index].active = None;
            self.pads.remove(&pad);
            self.pad_activity.remove(&pad);
            self.settle(index);
        } else {
            self.pads.remove(&pad);
            self.pad_activity.remove(&pad);
        }
    }

    pub fn feed(&mut self, event: DeviceEvent) {
        match event {
            DeviceEvent::PadConnected(pad) => self.connect(pad),
            DeviceEvent::PadDisconnected(pad) => self.disconnect(pad),
            DeviceEvent::PadButton { pad, button, value } => {
                self.connect(pad);
                let index = button.sdl_index();
                let down = value > 0.5;
                let Some(dev) = self.pads.get_mut(&pad) else {
                    return;
                };
                let held = dev.buttons & (1 << index) != 0;
                if down == held {
                    return;
                }
                dev.buttons ^= 1 << index;
                if down {
                    self.pad_activity.insert(pad, self.tick);
                    if let Some(at) = self.forwarding_slot(pad) {
                        if let Some(Action::Slot(number)) = self
                            .bindings_for(at)
                            .controller
                            .get(button.binding_name())
                        {
                            self.weapon_requests[at] = Some(*number);
                        }
                    }
                }
                self.update_pad_report(pad);
            }
            DeviceEvent::PadAxis { pad, axis, value } => {
                self.connect(pad);
                let at = axis as usize;
                let value = if value.is_nan() {
                    0.0
                } else {
                    value.clamp(-1.0, 1.0)
                };
                let Some(dev) = self.pads.get_mut(&pad) else {
                    return;
                };
                dev.axes[at] = if at >= 4 { value.max(0.0) } else { value };
                dev.axes_seen[at] = true;
                if value.abs() > PAD_ACTIVITY {
                    self.pad_activity.insert(pad, self.tick);
                }
                self.update_pad_report(pad);
            }
            DeviceEvent::MouseMotion { dx, dy } => self.look.add(dx, dy),
            DeviceEvent::MouseButton { button, down } => {
                if down {
                    self.mouse_down.insert(button.name());
                    self.mk_activity = self.tick;
                } else {
                    self.mouse_down.remove(button.name());
                }
            }
            DeviceEvent::Wheel { notches } => self.wheel.add(notches),
            DeviceEvent::Key { name, down } => {
                if down {
                    if self
                        .mk_bindings()
                        .keys
                        .get(&name)
                        .is_some_and(|action| *action != Action::None)
                    {
                        self.mk_activity = self.tick;
                    }
                    if self.keys_down.insert(name.clone()) {
                        if let Some(Action::Slot(number)) = self.mk_bindings().keys.get(&name) {
                            self.slot_keys.push(*number);
                        }
                    }
                } else {
                    self.keys_down.remove(&name);
                }
            }
            DeviceEvent::FocusLost => {
                self.keys_down.clear();
                self.slot_keys.clear();
                self.weapon_requests = [None; SLOTS];
                self.mouse_down.clear();
                self.pulses = [0; 2];
                self.look.take();
            }
        }
    }

    /// The slot that currently follows this pad on the guest queue.
    fn forwarding_slot(&self, pad: PadId) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.pad == Some(pad) && slot.active == Some(Active::Pad))
    }

    fn pad_input(&self, index: usize, pad: PadId) -> Option<PlayerInput> {
        self.pads.get(&pad)?;
        let report = self.pad_report(index, pad);
        let axes = report.axes.map(|value| value.unwrap_or_default());
        let mut held: ButtonBits = 0;
        for (at, bit) in SDL_BUTTON_BITS.iter().enumerate() {
            if report.buttons & (1 << at) != 0 {
                held |= bit;
            }
        }
        let triggers = [axis_from_sdl(axes[4]).max(0.0), axis_from_sdl(axes[5]).max(0.0)];
        for (trigger, bit) in triggers.into_iter().zip([bits::LT, bits::RT]) {
            if (to_i16(trigger.max(0.0)) >> 7) as u8 > crate::shim::ANALOG_THRESHOLD {
                held |= bit;
            }
        }
        Some(self.build(
            index,
            SourceKind::Pad(pad),
            held,
            Vec2::new(axis_from_sdl(axes[0]), -axis_from_sdl(axes[1])),
            Look::Stick(Vec2::new(axis_from_sdl(axes[2]), -axis_from_sdl(axes[3]))),
            triggers,
            AssistMask::PROFILE,
        ))
    }

    fn pad_report(&self, index: usize, pad: PadId) -> SdlReport {
        let Some(dev) = self.pads.get(&pad) else {
            return SdlReport::default();
        };
        let mut report = dev.report();
        let bindings = &self.bindings_for(index).controller;
        let mut movement = Vec2::new(dev.axes[0], dev.axes[1]);
        let mut walk = false;
        let mut triggers = [dev.axes[4], dev.axes[5]];
        for at in 0..SDL_BUTTON_BITS.len() {
            if dev.buttons & (1 << at) == 0 {
                continue;
            }
            let Some(button) = pad_button_from_index(at) else {
                continue;
            };
            let Some(action) = bindings.get(button.binding_name()).copied() else {
                continue;
            };
            // `report.buttons` is indexed by SDL button number; `default_bit` is
            // an Xbox guest mask. They only happen to line up for some controls.
            report.buttons &= !(1u16 << at);
            match action {
                Action::MoveForward => movement.y = 1.0,
                Action::MoveBack => movement.y = -1.0,
                Action::MoveLeft => movement.x = -1.0,
                Action::MoveRight => movement.x = 1.0,
                Action::Walk => walk = true,
                Action::Lt => triggers[0] = 1.0,
                Action::Rt => triggers[1] = 1.0,
                action => {
                    let guest_bit = action_button_bit(action);
                    if let Some(target) = SDL_BUTTON_BITS.iter().position(|bit| *bit == guest_bit) {
                        report.buttons |= 1u16 << target;
                    }
                }
            }
        }
        let mut movement = Vec2::new(movement.x.clamp(-1.0, 1.0), movement.y.clamp(-1.0, 1.0));
        if walk {
            movement = Vec2::new(movement.x * 0.5, movement.y * 0.5);
        }
        if dev.axes_seen[0] || movement.x != 0.0 {
            report.axes[0] = Some(to_i16(movement.x));
        }
        if dev.axes_seen[1] || movement.y != 0.0 {
            report.axes[1] = Some(sdl_axis_value(1, movement.y));
        }
        if dev.axes_seen[4] || triggers[0] != 0.0 {
            report.axes[4] = Some(to_i16(triggers[0].max(0.0)));
        }
        if dev.axes_seen[5] || triggers[1] != 0.0 {
            report.axes[5] = Some(to_i16(triggers[1].max(0.0)));
        }
        report
    }

    fn update_pad_report(&mut self, pad: PadId) {
        let Some(index) = self.forwarding_slot(pad) else {
            return;
        };
        let mut target = self.pad_report(index, pad);
        neutralize_missing_axes(&self.slots[index].sent, &mut target);
        let mut events = Vec::new();
        diff(&mut self.slots[index].sent, &target, &mut events);
        self.out
            .extend(events.into_iter().map(|event| (index as u8, event)));
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        &self,
        index: usize,
        source: SourceKind,
        held: ButtonBits,
        move_axis: Vec2,
        look: Look,
        triggers: [f32; 2],
        assists: AssistMask,
    ) -> PlayerInput {
        let slot = &self.slots[index];
        PlayerInput {
            step: slot.step,
            slot: index as u8,
            source,
            held,
            pressed: held & !slot.held_prev,
            released: slot.held_prev & !held,
            move_axis,
            look,
            triggers,
            menu: MenuIntent::default(),
            assists,
        }
    }

    fn mk_actions(&self) -> Vec<Action> {
        let b = self.mk_bindings();
        let mut actions: Vec<Action> = self
            .keys_down
            .iter()
            .filter_map(|name| b.keys.get(name))
            .copied()
            .collect();
        actions.extend(
            self.mouse_down
                .iter()
                .filter_map(|name| b.mouse.get(*name))
                .copied(),
        );
        for (pulse, name) in self.pulses.iter().zip(["WheelUp", "WheelDown"]) {
            if *pulse > 0 {
                actions.extend(b.mouse.get(name).copied());
            }
        }
        actions
    }

    fn mk_input(&self, index: usize, counts: (f64, f64)) -> PlayerInput {
        let actions = self.mk_actions();
        let has = |action: Action| actions.contains(&action);
        let axis = |plus: Action, minus: Action| {
            f32::from(i8::from(has(plus))) - f32::from(i8::from(has(minus)))
        };
        let mut mv = Vec2::new(
            axis(Action::MoveRight, Action::MoveLeft),
            axis(Action::MoveForward, Action::MoveBack),
        );
        let length = mv.x.hypot(mv.y);
        if length > 1.0 {
            mv = Vec2::new(mv.x / length, mv.y / length);
        }
        if has(Action::Walk) {
            mv = Vec2::new(mv.x * 0.5, mv.y * 0.5);
        }
        let triggers = [
            f32::from(u8::from(has(Action::Lt))),
            f32::from(u8::from(has(Action::Rt))),
        ];
        let mut held: ButtonBits = 0;
        for (action, bit) in [
            (Action::A, bits::A),
            (Action::B, bits::B),
            (Action::X, bits::X),
            (Action::Y, bits::Y),
            (Action::Start, bits::START),
            (Action::Back, bits::BACK),
            (Action::L3, bits::L3),
            (Action::R3, bits::R3),
            (Action::Black, bits::BLACK),
            (Action::White, bits::WHITE),
            (Action::Lt, bits::LT),
            (Action::Rt, bits::RT),
            (Action::DpadUp, bits::DPAD_UP),
            (Action::DpadDown, bits::DPAD_DOWN),
            (Action::DpadLeft, bits::DPAD_LEFT),
            (Action::DpadRight, bits::DPAD_RIGHT),
        ] {
            if has(action) {
                held |= bit;
            }
        }
        let (yaw_deg, pitch_deg) = self.bindings_for(index).sensitivity.to_degrees(counts);
        self.build(
            index,
            SourceKind::MouseKeyboard,
            held,
            mv,
            Look::Delta { yaw_deg, pitch_deg },
            triggers,
            AssistMask::NONE,
        )
    }

    /// Pending direct weapon slot requests (1..=9) per local slot, cleared by the call. The game side honours a request only
    /// when the entry is owned and selectable (docs/t-kbm-keymap.md); a request is never a state write.
    pub fn take_weapon_requests(&mut self) -> [Option<u8>; SLOTS] {
        std::mem::replace(&mut self.weapon_requests, [None; SLOTS])
    }

    /// One tick: fold the accumulators into one `PlayerInput` per active slot and queue the shim events of the mouse and
    /// keyboard slot. Call once per shell frame (later: once per simulation step, contract 7.1).
    pub fn tick(&mut self) -> [Option<PlayerInput>; SLOTS] {
        self.tick += 1;
        for pulse in &mut self.pulses {
            *pulse = pulse.saturating_sub(1);
        }
        let notches = self.wheel.take_whole();
        if notches != 0 {
            self.pulses[usize::from(notches < 0)] = WHEEL_PULSE_TICKS;
            self.mk_activity = self.tick;
        }
        let pressed_slot = self.slot_keys.last().copied();
        self.slot_keys.clear();
        let counts = self.look.take();
        if counts.0.abs() + counts.1.abs() >= MOTION_ACTIVITY_COUNTS {
            self.mk_activity = self.tick;
        }
        let hysteresis = self.config.hysteresis_ticks;
        for index in 0..SLOTS {
            let slot = &self.slots[index];
            if let (true, true, Some(pad), Some(active)) =
                (slot.auto, slot.mk, slot.pad, slot.active)
            {
                let pad_at = self.pad_activity.get(&pad).copied().unwrap_or(0);
                let (current, other, next) = match active {
                    Active::Mk => (self.mk_activity, pad_at, Active::Pad),
                    Active::Pad => (pad_at, self.mk_activity, Active::Mk),
                };
                if other > current && self.tick - slot.last_switch >= hysteresis {
                    self.switch(index, Some(next));
                }
            }
            // Route after arbitration: this same key press may have switched the owner from pad to keyboard.
            if self.slots[index].mk && self.slots[index].active == Some(Active::Mk) {
                if let Some(number) = pressed_slot {
                    self.weapon_requests[index] = Some(number);
                }
            }
            let input = match (self.slots[index].active, self.slots[index].pad) {
                (Some(Active::Pad), Some(pad)) => self.pad_input(index, pad),
                (Some(Active::Mk), _) => {
                    let input = self.mk_input(index, counts);
                    let mut events = Vec::new();
                    let queued = if self.config.raw_look && self.mounted & (1 << index) == 0 {
                        PlayerInput {
                            look: Look::Stick(Vec2::ZERO),
                            ..input
                        }
                    } else {
                        input
                    };
                    diff(
                        &mut self.slots[index].sent,
                        &report_from_player(&queued),
                        &mut events,
                    );
                    self.out
                        .extend(events.into_iter().map(|event| (index as u8, event)));
                    Some(input)
                }
                _ => None,
            };
            let slot = &mut self.slots[index];
            if let Some(input) = &input {
                slot.held_prev = input.held;
                slot.step += 1;
            }
            self.last[index] = input;
        }
        self.last
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_settings_tests {
    use super::*;
    use crate::settings::{Device, InputKind, InputSettings, SettingEdit};
    #[test]
    fn reassignment_selects_independent_look_actions_and_weapon_slots() {
        let mut router = Router::new(RouterConfig {
            raw_look: true,
            ..Default::default()
        });
        let mut settings = InputSettings::default();
        settings
            .apply_player(1, SettingEdit::SensitivityYaw(0.2))
            .unwrap();
        settings
            .apply_player(
                1,
                SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "KeyW".into(),
                    action: Action::A,
                },
            )
            .unwrap();
        settings
            .apply_player(
                1,
                SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "Digit1".into(),
                    action: Action::Slot(4),
                },
            )
            .unwrap();
        for player in [0, 1, 0] {
            settings
                .apply(SettingEdit::AssignDevice {
                    player,
                    device: Device::KeyboardMouse,
                })
                .unwrap();
            router.apply_settings(&settings);
            router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
            router.feed(DeviceEvent::Key {
                name: "KeyW".into(),
                down: true,
            });
            router.feed(DeviceEvent::Key {
                name: "Digit1".into(),
                down: true,
            });
            let input = router.tick()[player].unwrap();
            assert!(
                matches!(input.look, Look::Delta { yaw_deg, .. } if (yaw_deg - if player == 1 {20.0} else {2.2}).abs() < 0.001)
            );
            assert_eq!(input.held & bits::A != 0, player == 1);
            assert_eq!(input.move_axis.y, if player == 1 { 0.0 } else { 1.0 });
            assert_eq!(
                router.take_weapon_requests()[player],
                Some(if player == 1 { 4 } else { 1 })
            );
        }
    }

    #[test]
    fn live_edits_change_look_bindings_and_device_owner() {
        let mut router = Router::new(RouterConfig {
            raw_look: true,
            ..Default::default()
        });
        let mut settings = InputSettings::default();
        router.apply_settings(&settings);
        router.feed(DeviceEvent::MouseMotion {
            dx: 100.0,
            dy: 100.0,
        });
        let before = router.tick()[0].unwrap().look;
        settings.apply(SettingEdit::SensitivityYaw(0.1)).unwrap();
        settings.apply(SettingEdit::InvertPitch(true)).unwrap();
        settings
            .apply(SettingEdit::Rebind {
                kind: InputKind::Key,
                input: "KeyW".into(),
                action: Action::A,
            })
            .unwrap();
        router.apply_settings(&settings);
        router.feed(DeviceEvent::MouseMotion {
            dx: 100.0,
            dy: 100.0,
        });
        router.feed(DeviceEvent::Key {
            name: "KeyW".into(),
            down: true,
        });
        let after = router.tick()[0].unwrap();
        assert_ne!(before, after.look);
        assert!(
            matches!(after.look, Look::Delta { yaw_deg, .. } if (yaw_deg - 10.0).abs() < 0.001)
        );
        assert_ne!(after.held, 0);
        settings
            .apply(SettingEdit::AssignDevice {
                player: 1,
                device: Device::KeyboardMouse,
            })
            .unwrap();
        router.apply_settings(&settings);
        assert_eq!(router.mouse_owner(), Some(1));
        assert_eq!(router.source(0), None);
        assert_eq!(
            router.tick()[1].unwrap().held,
            0,
            "held inputs are cleared on reassignment"
        );
    }
    #[test]
    fn explicit_none_and_pad_assignments_survive_new_connections() {
        let mut router = Router::new(RouterConfig::default());
        let mut settings = InputSettings::default();
        settings.players = [Device::None, Device::Pad(0), Device::Pad(1), Device::None];
        router.apply_settings(&settings);
        router.feed(DeviceEvent::PadConnected(42));
        assert_eq!(router.source(0), None);
        assert_eq!(router.source(1), Some(InputSource::Pad(42)));
        router.feed(DeviceEvent::PadConnected(55));
        assert_eq!(router.source(2), Some(InputSource::Pad(55)));
    }
}

#[cfg(test)]
mod native_bindings_policy_tests {
    use super::*;
    #[test]
    fn profile_deactivation_restores_connected_devices_and_hot_swap() {
        let mut router = Router::new(RouterConfig {
            mouse_keyboard_slot: Some(0),
            hysteresis_ticks: 0,
            ..Default::default()
        });
        router.feed(DeviceEvent::PadConnected(42));
        let mut settings = crate::settings::InputSettings::default();
        settings.players = [crate::settings::Device::None; SLOTS];
        router.apply_settings(&settings);
        assert_eq!(router.source(0), None);
        router.restore_automatic_devices();
        assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
        router.tick();
        router.feed(DeviceEvent::PadButton {
            pad: 42,
            button: PadButton::South,
            value: 1.0,
        });
        router.tick();
        assert_eq!(router.source(0), Some(InputSource::Pad(42)));
        router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
        router.tick();
        assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
        router.feed(DeviceEvent::PadConnected(55));
        assert_eq!(router.source(1), Some(InputSource::Pad(55)));
        // Repeated automatic-policy ticks cannot clear held keys or reset owners.
        router.restore_automatic_devices();
        assert_eq!(router.source(1), Some(InputSource::Pad(55)));
    }
    #[test]
    fn changing_bindings_leaves_automatic_device_policy_intact() {
        let mut router = Router::new(RouterConfig {
            mouse_keyboard_slot: Some(0),
            hysteresis_ticks: 0,
            ..Default::default()
        });
        router.tick();
        router.feed(DeviceEvent::PadConnected(42));
        router.feed(DeviceEvent::PadButton {
            pad: 42,
            button: PadButton::South,
            value: 1.0,
        });
        router.tick();
        assert_eq!(router.source(0), Some(InputSource::Pad(42)));
        let mut bindings = Bindings::default();
        bindings.sensitivity.yaw_deg_per_count = 0.1;
        router.set_bindings(bindings);
        assert_eq!(router.source(0), Some(InputSource::Pad(42)));
        router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
        router.tick();
        assert_eq!(router.source(0), Some(InputSource::MouseKeyboard));
    }

    #[test]
    fn controller_button_rebind_changes_the_guest_and_rust_pad_state() {
        let mut router = Router::new(RouterConfig::default());
        let mut settings = crate::settings::InputSettings::default();
        settings.players = [
            crate::settings::Device::Pad(0),
            crate::settings::Device::None,
            crate::settings::Device::None,
            crate::settings::Device::None,
        ];
        settings.bindings.controller.insert("south".into(), Action::B);
        router.apply_settings(&settings);
        router.feed(DeviceEvent::PadConnected(42));
        router.feed(DeviceEvent::PadButton {
            pad: 42,
            button: PadButton::South,
            value: 1.0,
        });

        assert_eq!(router.drain_events(), vec![(0, ShimEvent::ButtonDown(1))]);
        assert_eq!(router.tick()[0].unwrap().held, bits::B);
    }
}
