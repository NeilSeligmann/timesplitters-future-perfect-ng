// SPDX-License-Identifier: GPL-3.0-or-later
//! SDL3 controller identity, per-device mapping and calibration.
//!
//! Profiles use a device serial when SDL provides one, otherwise the SDL device path, and finally
//! GUID plus same-GUID occurrence as a clearly weaker fallback. Runtime SDL instance IDs are not
//! persisted. The model is host-independent so event pumps and settings UI share one contract.

use std::collections::{BTreeMap, BTreeSet};

pub const BUTTON_COUNT: usize = 15;
pub const AXIS_COUNT: usize = 6;

/// Stable model identity when a host backend exposes no native controller GUID.
pub fn fallback_gamepad_guid(
    name: &str,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
) -> String {
    let identity = format!(
        "{:04x}:{:04x}:{name}",
        vendor_id.unwrap_or(0),
        product_id.unwrap_or(0)
    );
    let (mut a, mut b) = (0xcbf2_9ce4_8422_2325u64, 0x8422_2325_cbf2_9ce4u64);
    for byte in identity.bytes() {
        a = (a ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        b = (b ^ u64::from(byte))
            .rotate_left(7)
            .wrapping_mul(0x100_0000_01b3);
    }
    format!("{a:016x}{b:016x}")
}

/// Stable profile identity derived from SDL3 device metadata.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerIdentity {
    guid: String,
    key: IdentityKey,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum IdentityKey {
    Serial(String),
    Path(String),
    Occurrence(u8),
}

impl ControllerIdentity {
    /// Prefer physical serial, then SDL's device path, and use GUID occurrence only as fallback.
    pub fn from_sdl(
        guid: &str,
        serial: Option<&str>,
        path: Option<&str>,
        occurrence: u8,
    ) -> Result<Self, String> {
        let guid = normalize_guid(guid)?;
        let key = if let Some(serial) = nonempty_metadata(serial)? {
            IdentityKey::Serial(serial)
        } else if let Some(path) = nonempty_metadata(path)? {
            IdentityKey::Path(path)
        } else if occurrence < 8 {
            IdentityKey::Occurrence(occurrence)
        } else {
            return Err("same-GUID occurrence must be 0..7".into());
        };
        Ok(Self { guid, key })
    }

    pub fn guid(&self) -> &str {
        &self.guid
    }

    /// A canonical, delimiter-safe key for local settings files.
    pub fn profile_key(&self) -> String {
        match &self.key {
            IdentityKey::Serial(value) => {
                format!("{}-s{}", self.guid, encode_hex(value.as_bytes()))
            }
            IdentityKey::Path(value) => format!("{}-p{}", self.guid, encode_hex(value.as_bytes())),
            IdentityKey::Occurrence(value) => format!("{}-o{value}", self.guid),
        }
    }

    fn parse_profile_key(text: &str) -> Result<Self, String> {
        if text.len() < 35 || !text.is_ascii() {
            return Err("controller profile key is malformed".into());
        }
        let guid = normalize_guid(&text[..32])?;
        let suffix = &text[32..];
        let key = if let Some(encoded) = suffix.strip_prefix("-s") {
            IdentityKey::Serial(decode_hex_text(encoded)?)
        } else if let Some(encoded) = suffix.strip_prefix("-p") {
            IdentityKey::Path(decode_hex_text(encoded)?)
        } else if let Some(occurrence) = suffix.strip_prefix("-o") {
            let occurrence: u8 = occurrence
                .parse()
                .map_err(|_| "controller occurrence is malformed")?;
            if occurrence >= 8 {
                return Err("same-GUID occurrence must be 0..7".into());
            }
            IdentityKey::Occurrence(occurrence)
        } else {
            return Err("controller profile key has an unknown identity kind".into());
        };
        let identity = Self { guid, key };
        identity.validate()?;
        Ok(identity)
    }

    fn validate(&self) -> Result<(), String> {
        normalize_guid(&self.guid)?;
        match &self.key {
            IdentityKey::Serial(value) | IdentityKey::Path(value) => {
                nonempty_metadata(Some(value)).map(|_| ())
            }
            IdentityKey::Occurrence(value) if *value < 8 => Ok(()),
            IdentityKey::Occurrence(_) => Err("same-GUID occurrence must be 0..7".into()),
        }
    }
}

fn normalize_guid(guid: &str) -> Result<String, String> {
    if guid.len() != 32 || !guid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("SDL controller GUID must contain 32 hexadecimal digits".into());
    }
    Ok(guid.to_ascii_lowercase())
}

fn nonempty_metadata(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > 1024 || value.chars().any(char::is_control) {
        return Err("SDL controller serial/path must be printable and at most 1024 bytes".into());
    }
    Ok(Some(value.to_owned()))
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_hex_text(text: &str) -> Result<String, String> {
    if text.is_empty() || text.len() % 2 != 0 || !text.is_ascii() {
        return Err("controller identity metadata is malformed".into());
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = digit(pair[0]).ok_or("controller identity metadata must use lowercase hex")?;
        let low = digit(pair[1]).ok_or("controller identity metadata must use lowercase hex")?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes).map_err(|_| "controller identity metadata is not UTF-8".into())
}

/// Calibration for one physical SDL axis. Stick centers are measured; trigger axes begin at min.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisCalibration {
    pub min: i32,
    pub center: i32,
    pub max: i32,
    pub deadzone: u32,
    pub invert: bool,
}

impl AxisCalibration {
    fn default_for(axis: usize) -> Self {
        Self {
            min: if axis < 4 { -32768 } else { 0 },
            center: 0,
            max: 32767,
            deadzone: 0,
            invert: axis == 1 || axis == 3,
        }
    }

    fn validate(self, axis: usize) -> Result<(), String> {
        let trigger = axis >= 4;
        if self.min < if trigger { 0 } else { -32768 }
            || self.max > 32767
            || self.min >= self.max
            || (trigger && self.center != self.min)
            || (!trigger && (self.center <= self.min || self.center >= self.max))
            || self.deadzone >= 32767
        {
            return Err(format!(
                "axis {axis} has invalid min/center/max/deadzone calibration"
            ));
        }
        Ok(())
    }

    fn normalize(self, input: i16, trigger: bool) -> i16 {
        let value = i32::from(input).clamp(self.min, self.max);
        let mut normalized = if trigger {
            (i64::from(value - self.min) * 32767 / i64::from(self.max - self.min)) as i32
        } else if value >= self.center {
            (i64::from(value - self.center) * 32767 / i64::from(self.max - self.center)) as i32
        } else {
            -(i64::from(self.center - value) * 32768 / i64::from(self.center - self.min)) as i32
        };
        if self.invert {
            normalized = if trigger {
                32767 - normalized
            } else if normalized == i16::MIN.into() {
                32767
            } else {
                -normalized
            };
        }
        let magnitude = normalized.abs();
        let deadzone = self.deadzone as i32;
        if magnitude <= deadzone {
            return 0;
        }
        let maximum = if normalized < 0 { 32768 } else { 32767 };
        let scaled =
            i64::from(magnitude - deadzone) * i64::from(maximum) / i64::from(maximum - deadzone);
        (scaled as i32 * if normalized < 0 { -1 } else { 1 }) as i16
    }
}

/// The controller's physical-to-Xbox mapping, indexed by logical target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerProfile {
    /// Logical A..D-pad-right target to SDL physical button index; `None` disables a target.
    /// Logical GUIDE (target 14) must remain disabled because the game has no matching button.
    pub button_map: [Option<u8>; BUTTON_COUNT],
    /// Logical LX, LY, RX, RY, LT, RT target to physical SDL axis index.
    pub axis_map: [u8; AXIS_COUNT],
    /// Calibration is indexed by physical source axis.
    pub calibration: [AxisCalibration; AXIS_COUNT],
}

impl Default for ControllerProfile {
    fn default() -> Self {
        Self {
            // Logical XInput order -> SDL3 Gamepad physical button index. SDL's order is
            // A, B, X, Y, Back, Guide, Start, L3, R3, LB, RB, D-pad; the guest report
            // puts the bumpers before Start/Back and has no Guide button.
            button_map: [
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(9),
                Some(10),
                Some(6),
                Some(4),
                Some(7),
                Some(8),
                Some(11),
                Some(12),
                Some(13),
                Some(14),
                None,
            ],
            axis_map: std::array::from_fn(|axis| axis as u8),
            calibration: std::array::from_fn(AxisCalibration::default_for),
        }
    }
}

impl ControllerProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.button_map[14].is_some() {
            return Err("logical GUIDE target 14 must be disabled".into());
        }
        for (target, source) in self.button_map.iter().enumerate() {
            if source.is_some_and(|source| usize::from(source) >= BUTTON_COUNT) {
                return Err(format!("button {target} source must be -1..14"));
            }
        }
        for (target, source) in self.axis_map.iter().copied().enumerate() {
            if usize::from(source) >= AXIS_COUNT || (target < 4) != (source < 4) {
                return Err(format!(
                    "axis {target} source must stay in its stick/trigger class"
                ));
            }
        }
        for source in 0..AXIS_COUNT {
            self.calibration[source].validate(source)?;
        }
        Ok(())
    }

    /// Apply one SDL sample to the logical controller report used by the guest input adapter.
    pub fn apply(
        &self,
        buttons: &[bool; BUTTON_COUNT],
        axes: &[i16; AXIS_COUNT],
    ) -> Result<ControllerReport, String> {
        self.validate()?;
        let mut report = ControllerReport::default();
        for (target, source) in self.button_map.iter().enumerate().take(14) {
            report.buttons[target] = source.is_some_and(|source| buttons[usize::from(source)]);
        }
        for target in 0..AXIS_COUNT {
            let source = usize::from(self.axis_map[target]);
            report.axes[target] = self.calibration[source].normalize(axes[source], target >= 4);
        }
        Ok(report)
    }
}

/// One logical Xbox controller sample. Button order is A, B, X, Y, LB, RB, Start, Back, L3, R3,
/// D-pad up/down/left/right; axes are LX, LY, RX, RY, LT, RT.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControllerReport {
    pub buttons: [bool; 14],
    pub axes: [i16; AXIS_COUNT],
}

/// Profiles keyed by a physical identity, with an unmodified default for devices seen the first time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerProfiles {
    profiles: BTreeMap<ControllerIdentity, ControllerProfile>,
}

#[derive(Clone, Debug)]
struct ControllerState {
    identity: ControllerIdentity,
    physical_buttons: [bool; BUTTON_COUNT],
    physical_axes: [i16; AXIS_COUNT],
    report: ControllerReport,
}

/// Applies per-device profiles to SDL controller events before feeding the native input router.
/// The runtime id is session-only; profile selection always uses the inventory's stable identity.
#[derive(Clone, Debug, Default)]
pub struct ControllerEventPath {
    inventory: ControllerInventory,
    profiles: ControllerProfiles,
    states: BTreeMap<u32, ControllerState>,
}

impl ControllerEventPath {
    pub fn profiles(&self) -> &ControllerProfiles {
        &self.profiles
    }

    pub fn profile_text(&self) -> String {
        self.profiles.to_text()
    }

    pub fn load_profile_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        match std::fs::read_to_string(path) {
            Ok(text) => self.load_profiles(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn persist_profile_file(&self, path: &std::path::Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| "controller profile path has no parent".to_owned())?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, self.profile_text())
            .and_then(|()| std::fs::rename(&temporary, path))
            .map_err(|error| {
                let _ = std::fs::remove_file(&temporary);
                error.to_string()
            })
    }

    /// Snapshot live devices and profiles for the settings UI. Runtime IDs remain session-only.
    pub fn settings_snapshot(&self) -> (Vec<ConnectedController>, ControllerProfiles) {
        (
            self.inventory.iter().cloned().collect(),
            self.profiles.clone(),
        )
    }

    pub fn load_profiles(&mut self, text: &str) -> Result<(), String> {
        let profiles = ControllerProfiles::parse(text)?;
        self.profiles = profiles;
        Ok(())
    }

    pub fn connect(
        &mut self,
        router: &mut crate::router::Router,
        runtime_id: u32,
        guid: &str,
        serial: Option<&str>,
        path: Option<&str>,
        name: &str,
    ) -> Result<ControllerIdentity, String> {
        let device = self
            .inventory
            .connect(runtime_id, guid, serial, path, name)?
            .clone();
        let profile = self.profiles.profile(&device.identity);
        let state = ControllerState {
            identity: device.identity.clone(),
            physical_buttons: [false; BUTTON_COUNT],
            physical_axes: [0; AXIS_COUNT],
            report: profile.apply(&[false; BUTTON_COUNT], &[0; AXIS_COUNT])?,
        };
        self.states.insert(runtime_id, state);
        router.feed(crate::router::DeviceEvent::PadConnected(runtime_id));
        Ok(device.identity)
    }

    pub fn disconnect(&mut self, router: &mut crate::router::Router, runtime_id: u32) -> bool {
        if self.inventory.disconnect(runtime_id).is_none() {
            return false;
        }
        self.states.remove(&runtime_id);
        router.feed(crate::router::DeviceEvent::PadDisconnected(runtime_id));
        true
    }

    pub fn set_button(
        &mut self,
        router: &mut crate::router::Router,
        runtime_id: u32,
        physical_button: u8,
        down: bool,
    ) -> Result<ControllerReport, String> {
        let button = usize::from(physical_button);
        if button >= BUTTON_COUNT {
            return Err(format!("physical button must be 0..{}", BUTTON_COUNT - 1));
        }
        let state = self
            .states
            .get_mut(&runtime_id)
            .ok_or_else(|| format!("SDL runtime id {runtime_id} is not connected"))?;
        state.physical_buttons[button] = down;
        Self::route_report(runtime_id, &self.profiles, state, router, None)
    }

    pub fn set_axis(
        &mut self,
        router: &mut crate::router::Router,
        runtime_id: u32,
        physical_axis: u8,
        value: i16,
    ) -> Result<ControllerReport, String> {
        let axis = usize::from(physical_axis);
        if axis >= AXIS_COUNT {
            return Err(format!("physical axis must be 0..{}", AXIS_COUNT - 1));
        }
        let state = self
            .states
            .get_mut(&runtime_id)
            .ok_or_else(|| format!("SDL runtime id {runtime_id} is not connected"))?;
        state.physical_axes[axis] = value;
        Self::route_report(runtime_id, &self.profiles, state, router, Some(axis))
    }

    pub fn set_profile(
        &mut self,
        router: &mut crate::router::Router,
        identity: ControllerIdentity,
        profile: ControllerProfile,
    ) -> Result<(), String> {
        self.profiles.set(identity.clone(), profile)?;
        for (runtime_id, state) in self
            .states
            .iter_mut()
            .filter(|(_, state)| state.identity == identity)
        {
            Self::route_report(*runtime_id, &self.profiles, state, router, None)?;
        }
        Ok(())
    }

    fn route_report(
        runtime_id: u32,
        profiles: &ControllerProfiles,
        state: &mut ControllerState,
        router: &mut crate::router::Router,
        changed_physical_axis: Option<usize>,
    ) -> Result<ControllerReport, String> {
        let profile = profiles.profile(&state.identity);
        let report = profile.apply(&state.physical_buttons, &state.physical_axes)?;
        for (target, (was_down, is_down)) in state
            .report
            .buttons
            .iter()
            .zip(report.buttons.iter())
            .enumerate()
        {
            if was_down != is_down {
                router.feed(crate::router::DeviceEvent::PadButton {
                    pad: runtime_id,
                    button: logical_button(target)?,
                    value: if *is_down { 1.0 } else { 0.0 },
                });
            }
        }
        for (axis, (before, after)) in state.report.axes.iter().zip(report.axes.iter()).enumerate()
        {
            let source = usize::from(profile.axis_map[axis]);
            if before != after || changed_physical_axis == Some(source) {
                router.feed(crate::router::DeviceEvent::PadAxis {
                    pad: runtime_id,
                    axis: logical_axis(axis)?,
                    value: axis_to_router(*after, axis >= 4),
                });
            }
        }
        state.report = report;
        Ok(report)
    }
}

fn logical_button(target: usize) -> Result<crate::router::PadButton, String> {
    use crate::router::PadButton as Button;
    Ok(match target {
        0 => Button::South,
        1 => Button::East,
        2 => Button::West,
        3 => Button::North,
        4 => Button::LeftBumper,
        5 => Button::RightBumper,
        6 => Button::Start,
        7 => Button::Select,
        8 => Button::LeftThumb,
        9 => Button::RightThumb,
        10 => Button::DPadUp,
        11 => Button::DPadDown,
        12 => Button::DPadLeft,
        13 => Button::DPadRight,
        _ => {
            return Err(format!(
                "logical controller button {target} has no guest input"
            ));
        }
    })
}

fn logical_axis(axis: usize) -> Result<crate::router::PadAxis, String> {
    use crate::router::PadAxis as Axis;
    Ok(match axis {
        0 => Axis::LeftX,
        1 => Axis::LeftY,
        2 => Axis::RightX,
        3 => Axis::RightY,
        4 => Axis::LeftTrigger,
        5 => Axis::RightTrigger,
        _ => return Err(format!("logical controller axis {axis} is invalid")),
    })
}

fn axis_to_router(value: i16, trigger: bool) -> f32 {
    if trigger {
        f32::from(value.max(0)) / f32::from(i16::MAX)
    } else if value == i16::MIN {
        -1.0
    } else {
        f32::from(value) / f32::from(i16::MAX)
    }
}

/// One live SDL gamepad and its stable profile identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedController {
    pub runtime_id: u32,
    pub identity: ControllerIdentity,
    pub name: String,
}

/// Session inventory translating ephemeral SDL ids to persisted controller identities.
///
/// Serial and path identities remain stable across enumeration order changes. Devices with no
/// unique metadata receive the lowest free same-GUID occurrence for this connection session;
/// two indistinguishable devices cannot be matched to physical units across reconnects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ControllerInventory {
    connected: BTreeMap<u32, ConnectedController>,
}

impl ControllerInventory {
    /// Register a connected gamepad. Duplicate runtime ids and duplicate stable metadata are
    /// rejected rather than silently attaching one device's profile to another.
    pub fn connect(
        &mut self,
        runtime_id: u32,
        guid: &str,
        serial: Option<&str>,
        path: Option<&str>,
        name: &str,
    ) -> Result<&ConnectedController, String> {
        if self.connected.contains_key(&runtime_id) {
            return Err(format!("SDL runtime id {runtime_id} is already connected"));
        }
        normalize_guid(guid)?;
        let name = nonempty_metadata(Some(name))?.unwrap_or_else(|| "Unnamed controller".into());
        let identity = if serial.is_some_and(|value| !value.is_empty())
            || path.is_some_and(|value| !value.is_empty())
        {
            ControllerIdentity::from_sdl(guid, serial, path, 0)?
        } else {
            let occurrence = (0..8)
                .find(|occurrence| {
                    ControllerIdentity::from_sdl(guid, None, None, *occurrence).is_ok_and(
                        |candidate| {
                            !self
                                .connected
                                .values()
                                .any(|device| device.identity == candidate)
                        },
                    )
                })
                .ok_or("at most eight same-GUID devices can be distinguished in one session")?;
            ControllerIdentity::from_sdl(guid, None, None, occurrence)?
        };
        if self
            .connected
            .values()
            .any(|device| device.identity == identity)
        {
            return Err(format!(
                "controller identity {} is already connected",
                identity.profile_key()
            ));
        }
        self.connected.insert(
            runtime_id,
            ConnectedController {
                runtime_id,
                identity,
                name,
            },
        );
        Ok(self
            .connected
            .get(&runtime_id)
            .expect("inserted controller"))
    }

    /// Remove a disconnected SDL runtime id, returning the identity that was assigned to it.
    pub fn disconnect(&mut self, runtime_id: u32) -> Option<ConnectedController> {
        self.connected.remove(&runtime_id)
    }

    pub fn get(&self, runtime_id: u32) -> Option<&ConnectedController> {
        self.connected.get(&runtime_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ConnectedController> {
        self.connected.values()
    }

    pub fn len(&self) -> usize {
        self.connected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connected.is_empty()
    }
}

impl ControllerProfiles {
    /// Whether a device has an explicitly stored profile rather than the built-in default.
    pub fn contains(&self, identity: &ControllerIdentity) -> bool {
        self.profiles.contains_key(identity)
    }

    pub fn profile(&self, identity: &ControllerIdentity) -> ControllerProfile {
        self.profiles.get(identity).cloned().unwrap_or_default()
    }

    pub fn get(&self, identity: &ControllerIdentity) -> Option<&ControllerProfile> {
        self.profiles.get(identity)
    }

    pub fn set(
        &mut self,
        identity: ControllerIdentity,
        profile: ControllerProfile,
    ) -> Result<(), String> {
        identity.validate()?;
        profile.validate()?;
        self.profiles.insert(identity, profile);
        Ok(())
    }

    pub fn remove(&mut self, identity: &ControllerIdentity) -> bool {
        self.profiles.remove(identity).is_some()
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Deterministic text format; unknown versions and fields are rejected instead of ignored.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut profiles = BTreeMap::new();
        let mut version_seen = false;
        let mut current: Option<(ControllerIdentity, ControllerProfile, BTreeSet<String>)> = None;
        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(section) = line
                .strip_prefix("[device.")
                .and_then(|v| v.strip_suffix(']'))
            {
                if let Some((identity, profile, _)) = current.take() {
                    profile
                        .validate()
                        .map_err(|e| format!("line {line_no}: {e}"))?;
                    if profiles.insert(identity, profile).is_some() {
                        return Err(format!("line {}: duplicate device section", line_no + 1));
                    }
                }
                current = Some((
                    ControllerIdentity::parse_profile_key(section)
                        .map_err(|e| format!("line {}: {e}", line_no + 1))?,
                    ControllerProfile::default(),
                    BTreeSet::new(),
                ));
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {}: expected key=value", line_no + 1))?;
            let key = key.trim();
            let value = value.trim();
            if let Some((_, profile, seen)) = current.as_mut() {
                if !seen.insert(key.to_owned()) {
                    return Err(format!("line {}: duplicate {key:?}", line_no + 1));
                }
                match key {
                    "buttons" => profile.button_map = parse_button_map(value)?,
                    "axes" => profile.axis_map = parse_axis_map(value)?,
                    _ if key.starts_with("axis.") => {
                        let axis: usize = key[5..]
                            .parse()
                            .map_err(|_| format!("line {}: invalid axis key", line_no + 1))?;
                        if axis >= AXIS_COUNT {
                            return Err(format!("line {}: axis index out of range", line_no + 1));
                        }
                        profile.calibration[axis] = parse_calibration(value)?;
                    }
                    _ => {
                        return Err(format!(
                            "line {}: unknown controller profile field {key:?}",
                            line_no + 1
                        ));
                    }
                }
            } else if key == "version" && !version_seen {
                if value != "1" {
                    return Err(format!(
                        "line {}: unsupported controller profile version",
                        line_no + 1
                    ));
                }
                version_seen = true;
            } else {
                return Err(format!(
                    "line {}: expected one version=1 before device sections",
                    line_no + 1
                ));
            }
        }
        if !version_seen {
            return Err("controller profiles are missing version=1".into());
        }
        if let Some((identity, profile, _)) = current {
            profile.validate()?;
            if profiles.insert(identity, profile).is_some() {
                return Err("duplicate device section".into());
            }
        }
        Ok(Self { profiles })
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("version=1\n");
        for (identity, profile) in &self.profiles {
            out.push_str("\n[device.");
            out.push_str(&identity.profile_key());
            out.push_str("]\nbuttons=");
            out.push_str(
                &profile
                    .button_map
                    .iter()
                    .map(|v| v.map_or(-1, i16::from).to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            out.push_str("\naxes=");
            out.push_str(
                &profile
                    .axis_map
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            for (axis, calibration) in profile.calibration.iter().enumerate() {
                out.push_str(&format!(
                    "\naxis.{axis}={},{},{},{},{}",
                    calibration.min,
                    calibration.center,
                    calibration.max,
                    calibration.deadzone,
                    calibration.invert
                ));
            }
            out.push('\n');
        }
        out
    }
}

fn parse_button_map(value: &str) -> Result<[Option<u8>; BUTTON_COUNT], String> {
    let parsed = parse_integer_list::<BUTTON_COUNT>(value)?;
    let mut mapping = [None; BUTTON_COUNT];
    for (target, item) in parsed.into_iter().enumerate() {
        mapping[target] = match item {
            -1 => None,
            0..=14 => Some(item as u8),
            _ => return Err("button mapping values must be -1..14".into()),
        };
    }
    Ok(mapping)
}

fn parse_axis_map(value: &str) -> Result<[u8; AXIS_COUNT], String> {
    let parsed = parse_integer_list::<AXIS_COUNT>(value)?;
    let mut mapping = [0; AXIS_COUNT];
    for (target, item) in parsed.into_iter().enumerate() {
        mapping[target] =
            u8::try_from(item).map_err(|_| "axis mapping values must be 0..5".to_owned())?;
    }
    Ok(mapping)
}

fn parse_integer_list<const N: usize>(value: &str) -> Result<[i32; N], String> {
    let values: Vec<i32> = value
        .split(',')
        .map(|part| {
            part.trim()
                .parse()
                .map_err(|_| "invalid integer list".to_owned())
        })
        .collect::<Result<_, _>>()?;
    values
        .try_into()
        .map_err(|_| format!("expected {} integer values", N))
}

fn parse_calibration(value: &str) -> Result<AxisCalibration, String> {
    let parts: Vec<_> = value.split(',').map(str::trim).collect();
    if parts.len() != 5 {
        return Err("calibration needs min,center,max,deadzone,invert".into());
    }
    Ok(AxisCalibration {
        min: parts[0].parse().map_err(|_| "invalid calibration min")?,
        center: parts[1].parse().map_err(|_| "invalid calibration center")?,
        max: parts[2].parse().map_err(|_| "invalid calibration max")?,
        deadzone: parts[3]
            .parse()
            .map_err(|_| "invalid calibration deadzone")?,
        invert: parts[4].parse().map_err(|_| "invalid calibration invert")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = "00112233445566778899aabbccddeeff";

    #[test]
    fn identity_prefers_serial_then_path_then_occurrence_and_round_trips() {
        let serial =
            ControllerIdentity::from_sdl(GUID, Some("unit-1"), Some("usb/1-2"), 0).unwrap();
        let path = ControllerIdentity::from_sdl(GUID, None, Some("usb/1-2"), 1).unwrap();
        let occurrence = ControllerIdentity::from_sdl(GUID, None, None, 1).unwrap();
        assert_ne!(serial, path);
        assert_ne!(path, occurrence);
        assert!(serial.profile_key().contains("-s"));
        assert!(path.profile_key().contains("-p"));
        assert!(occurrence.profile_key().ends_with("-o1"));
        for identity in [serial, path, occurrence] {
            assert_eq!(
                ControllerIdentity::parse_profile_key(&identity.profile_key()).unwrap(),
                identity
            );
        }
        let uppercase =
            ControllerIdentity::from_sdl(&GUID.to_ascii_uppercase(), None, None, 0).unwrap();
        assert_eq!(uppercase.guid(), GUID);
        assert!(ControllerIdentity::from_sdl("bad", None, None, 0).is_err());
        assert!(ControllerIdentity::from_sdl(GUID, None, None, 8).is_err());
    }

    #[test]
    fn inventory_assigns_stable_metadata_and_distinct_identical_device_slots() {
        let mut inventory = ControllerInventory::default();
        let first = inventory
            .connect(7, GUID, None, None, "Controller")
            .unwrap()
            .identity
            .clone();
        let second = inventory
            .connect(3, GUID, Some("serial-b"), None, "Controller B")
            .unwrap()
            .identity
            .clone();
        let third = inventory
            .connect(9, GUID, None, None, "Controller")
            .unwrap()
            .identity
            .clone();
        assert!(first.profile_key().ends_with("-o0"));
        assert!(third.profile_key().ends_with("-o1"));
        assert!(second.profile_key().contains("-s"));
        assert_eq!(inventory.len(), 3);

        let removed = inventory.disconnect(7).unwrap();
        assert_eq!(removed.identity, first);
        assert!(inventory.get(7).is_none());
        assert!(inventory.disconnect(7).is_none());
        let reconnected = inventory
            .connect(11, GUID, None, None, "Controller")
            .unwrap();
        assert_eq!(reconnected.identity, first);
    }

    #[test]
    fn inventory_rejects_runtime_and_stable_identity_collisions() {
        let mut inventory = ControllerInventory::default();
        inventory
            .connect(1, GUID, Some("serial"), None, "Controller")
            .unwrap();
        assert!(
            inventory
                .connect(1, GUID, Some("other"), None, "Other")
                .unwrap_err()
                .contains("runtime id 1")
        );
        assert!(
            inventory
                .connect(2, GUID, Some("serial"), None, "Duplicate")
                .unwrap_err()
                .contains("identity")
        );
        assert!(
            inventory
                .connect(3, "bad-guid", None, None, "Invalid")
                .unwrap_err()
                .contains("32 hexadecimal digits")
        );
        assert_eq!(inventory.len(), 1);
    }

    #[test]
    fn default_profile_preserves_standard_sdl_to_xbox_mapping() {
        let profile = ControllerProfile::default();
        assert_eq!(
            profile.button_map,
            [
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(9),
                Some(10),
                Some(6),
                Some(4),
                Some(7),
                Some(8),
                Some(11),
                Some(12),
                Some(13),
                Some(14),
                None,
            ]
        );
        let buttons = std::array::from_fn(|index| matches!(index, 0 | 6 | 8 | 10 | 14));
        let axes = [-32768, -32768, 32767, 32767, 1, 32767];
        let report = profile.apply(&buttons, &axes).unwrap();
        assert!(report.buttons[0]); // A
        assert!(report.buttons[6]); // Start
        assert!(report.buttons[9]); // Right thumb
        assert!(report.buttons[5]); // Right bumper
        assert!(report.buttons[13]); // D-pad right
        assert!(!report.buttons[4]); // Left bumper is not the SDL Back button.
        assert!(!report.buttons[7]); // Back is not the SDL left-stick button.
        assert_eq!(report.axes, [-32768, 32767, 32767, -32767, 1, 32767]);
    }

    #[test]
    fn event_path_applies_per_device_mapping_before_the_router() {
        use crate::router::{Router, RouterConfig};
        use crate::shim::ShimEvent;

        let mut path = ControllerEventPath::default();
        let mut router = Router::new(RouterConfig::default());
        let identity = path
            .connect(
                &mut router,
                42,
                GUID,
                Some("serial-a"),
                None,
                "Controller A",
            )
            .unwrap();
        assert!(router.drain_events().is_empty());

        // The legacy SDL adapter reports the first sample even when it is neutral. Preserve that
        // edge so the input queue's axis state is initialized identically on both paths.
        path.set_axis(&mut router, 42, 0, 0).unwrap();
        assert_eq!(
            router.drain_events(),
            vec![(0, ShimEvent::Axis { axis: 0, value: 0 })]
        );

        let report = path.set_button(&mut router, 42, 9, true).unwrap();
        assert!(report.buttons[4]); // Physical SDL left bumper -> logical Xbox left bumper.
        assert_eq!(router.drain_events(), vec![(0, ShimEvent::ButtonDown(9))]);

        let mut profile = ControllerProfile::default();
        profile.button_map[4] = Some(0);
        path.set_profile(&mut router, identity, profile).unwrap();
        assert_eq!(router.drain_events(), vec![(0, ShimEvent::ButtonUp(9))]);

        let report = path.set_axis(&mut router, 42, 0, i16::MAX).unwrap();
        assert_eq!(report.axes[0], i16::MAX);
        assert_eq!(
            router.drain_events(),
            vec![(
                0,
                ShimEvent::Axis {
                    axis: 0,
                    value: i16::MAX
                }
            )]
        );
        assert!(path.disconnect(&mut router, 42));
        assert!(!path.disconnect(&mut router, 42));
        assert!(path.set_button(&mut router, 42, 9, false).is_err());
    }

    #[test]
    fn profiles_are_isolated_per_physical_device_and_round_trip() {
        let first = ControllerIdentity::from_sdl(GUID, Some("first"), None, 0).unwrap();
        let second = ControllerIdentity::from_sdl(GUID, Some("second"), None, 1).unwrap();
        let mut profile = ControllerProfile::default();
        profile.button_map[0] = Some(1);
        profile.calibration[0].deadzone = 1200;
        let mut profiles = ControllerProfiles::default();
        profiles.set(first.clone(), profile.clone()).unwrap();
        assert_eq!(profiles.profile(&first), profile);
        assert_eq!(profiles.profile(&second), ControllerProfile::default());
        let decoded = ControllerProfiles::parse(&profiles.to_text()).unwrap();
        assert_eq!(decoded, profiles);
    }

    #[test]
    fn calibration_rescales_deadzone_and_clamps_asymmetric_extremes() {
        let mut profile = ControllerProfile::default();
        profile.calibration[0].deadzone = 2000;
        let mut buttons = [false; BUTTON_COUNT];
        let mut axes = [0; AXIS_COUNT];
        axes[0] = 1000;
        assert_eq!(profile.apply(&buttons, &axes).unwrap().axes[0], 0);
        axes[0] = i16::MIN;
        assert_eq!(profile.apply(&buttons, &axes).unwrap().axes[0], i16::MIN);
        axes[0] = i16::MAX;
        assert_eq!(profile.apply(&buttons, &axes).unwrap().axes[0], i16::MAX);
        profile.button_map[0] = Some(1);
        buttons[1] = true;
        assert!(profile.apply(&buttons, &axes).unwrap().buttons[0]);
        profile.axis_map[0] = 4;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn malformed_or_duplicate_profile_fields_are_rejected() {
        let key = ControllerIdentity::from_sdl(GUID, None, None, 0)
            .unwrap()
            .profile_key();
        assert!(ControllerProfiles::parse("").is_err());
        assert!(ControllerProfiles::parse(&format!("version=2\n[device.{key}]\n")).is_err());
        assert!(
            ControllerProfiles::parse(&format!("version=1\n[device.{key}]\nbuttons=0\n")).is_err()
        );
        assert!(
            ControllerProfiles::parse(&format!("version=1\n[device.{key}]\nunknown=1\n")).is_err()
        );
        assert!(ControllerProfiles::parse(&format!(
            "version=1\n[device.{key}]\nbuttons=-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1\nbuttons=-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1\n"
        )).is_err());
        assert!(
            ControllerProfiles::parse(&format!(
                "version=1\n[device.{key}]\nbuttons=-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,0\n"
            ))
            .is_err()
        );
        let mut profile = ControllerProfile::default();
        profile.calibration[0].deadzone = 32767;
        assert!(profile.validate().is_err());
    }
}
