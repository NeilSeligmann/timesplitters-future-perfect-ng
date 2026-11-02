// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned native input route records (T2348, format: docs/t2348-input-recording.md).
//!
//! A tick stores the four typed `PlayerInput` values after the Rust router has applied keyboard,
//! mouse and controller events. Float values are encoded by their IEEE-754 bits, so replay does
//! not round through text. The caller supplies the fixed simulation tick, host poll index and
//! monotonic wall time; this module does not invent or advance any of those clocks.

use crate::player::{AssistMask, Look, MenuIntent, PlayerInput, SourceKind, Vec2};
use std::io::{self, BufRead, Write};

pub const FORMAT_VERSION: u32 = 6;
/// T3820: the first version whose `D` rows carry every host device event (native-page input).
pub const DEVICE_ROWS_VERSION: u32 = 6;
pub const SIMULATION_HZ: u32 = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    pub sim_tick: u64,
    pub host_poll: u64,
    pub wall_ns: u64,
    pub inputs: [Option<PlayerInput>; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub sim_tick: u64,
    pub host_poll: u64,
    pub wall_ns: u64,
    pub label: String,
}

/// Semantic event emitted by the native `UiInput` router. IDs remain opaque typed-state keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEvent {
    Focus(u64),
    Activate(u64),
    Back,
    Start,
    Scroll(u32, i32),
    Hover(u64),
    HoverLeave,
    TextInput(char),
    TextCommit { widget: u64, text: Vec<u8> },
    TextCancel(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ui {
    pub sim_tick: u64,
    pub host_poll: u64,
    pub wall_ns: u64,
    pub context: u8,
    pub player: u8,
    /// Rust-owned page builder identity. Zero is reserved for the standalone main-menu model.
    pub page: u32,
    pub event: UiEvent,
}

/// T3392 (format v5): the mouse pointer as the shell saw it at one guest frame boundary, keyed by
/// host poll like the other rows. `position` is the design grid (640x480, f32 bits on disk), `None`
/// when the cursor is outside the window. `buttons` is a `BUTTON_*` mask of the held mouse buttons.
/// Rows are written when the position or the buttons changed. Replay feeds the position to the same
/// Map Maker pointer path the live mouse uses; the click itself is also in the `T` rows (the router's
/// virtual pad A), so the button mask is a record of the device, not a second delivery.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    pub sim_tick: u64,
    pub host_poll: u64,
    pub wall_ns: u64,
    pub buttons: u8,
    pub position: Option<(f32, f32)>,
}

pub const BUTTON_LEFT: u8 = 1;
pub const BUTTON_RIGHT: u8 = 2;
pub const BUTTON_MIDDLE: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    Tick(Tick),
    Mark(Mark),
    Ui(Ui),
    Pointer(Pointer),
    /// T3592: the physical pad while a native page owned input (`T` rows hold `-` then). Diagnostic and
    /// repair data, playback ignores it.
    NativePad(Tick),
    /// T3820 (format v6): one host device event at the single Bevy intake point (`tsfp-input-intake` codec).
    Device(Payload),
    /// T3819 (format v6): a periodic state digest written by the desync detector; playback ignores it.
    State(Payload),
}

/// T3820/T3819 (format v6): a clocked row whose payload is owned by another module (`D` device events,
/// `S` state digests). The payload has no tab, CR or LF; `S` payloads carry their own `v=<n>;` prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub sim_tick: u64,
    pub host_poll: u64,
    pub wall_ns: u64,
    pub payload: String,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input recording I/O: {error}"),
            Self::Invalid(message) => write!(f, "invalid input recording: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Stream writer. A final `E` line is required, so a crash-truncated file is rejected on replay.
pub struct Writer<W> {
    output: W,
    last_tick: Option<u64>,
    ended: bool,
}

impl<W: Write> Writer<W> {
    pub fn new(output: W, unlock_all: bool) -> Result<Self, Error> {
        Self::stamped(output, unlock_all, None)
    }

    /// T3820: the header also names the build that recorded (`build=<git sha>`, ignored by playback).
    pub fn stamped(mut output: W, unlock_all: bool, build: Option<&str>) -> Result<Self, Error> {
        write!(
            output,
            "TSFP-INPUT-REC\t{FORMAT_VERSION}\t{SIMULATION_HZ}\tunlock-all={}",
            u8::from(unlock_all)
        )?;
        if let Some(build) = build.filter(|build| !build.is_empty()) {
            check_payload(build)?;
            write!(output, "\tbuild={build}")?;
        }
        writeln!(output)?;
        Ok(Self {
            output,
            last_tick: None,
            ended: false,
        })
    }

    pub fn tick(&mut self, tick: &Tick) -> Result<(), Error> {
        self.ensure_open()?;
        self.check_tick(tick.sim_tick)?;
        write!(
            self.output,
            "T\t{}\t{}\t{}",
            tick.sim_tick, tick.host_poll, tick.wall_ns
        )?;
        for input in &tick.inputs {
            write!(self.output, "\t{}", encode_input(*input))?;
        }
        writeln!(self.output)?;
        self.last_tick = Some(tick.sim_tick);
        Ok(())
    }

    /// T3592: `G` row, the same fields as `T`, written for ticks a native page owned (those `T` rows are `-`).
    pub fn native_pad(&mut self, tick: &Tick) -> Result<(), Error> {
        self.ensure_open()?;
        write!(
            self.output,
            "G\t{}\t{}\t{}",
            tick.sim_tick, tick.host_poll, tick.wall_ns
        )?;
        for input in &tick.inputs {
            write!(self.output, "\t{}", encode_input(*input))?;
        }
        writeln!(self.output)?;
        Ok(())
    }

    pub fn mark(&mut self, mark: &Mark) -> Result<(), Error> {
        self.ensure_open()?;
        let label = mark
            .label
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        writeln!(
            self.output,
            "M\t{}\t{}\t{}\t{}",
            mark.sim_tick, mark.host_poll, mark.wall_ns, label
        )?;
        Ok(())
    }

    pub fn ui(&mut self, event: &Ui) -> Result<(), Error> {
        self.ensure_open()?;
        writeln!(
            self.output,
            "U\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            event.sim_tick,
            event.host_poll,
            event.wall_ns,
            event.context,
            event.player,
            event.page,
            encode_ui(&event.event)
        )?;
        Ok(())
    }

    pub fn pointer(&mut self, event: &Pointer) -> Result<(), Error> {
        self.ensure_open()?;
        let (x, y) = event.position.map_or_else(
            || ("-".to_string(), "-".to_string()),
            |(x, y)| {
                (
                    format!("{:08x}", x.to_bits()),
                    format!("{:08x}", y.to_bits()),
                )
            },
        );
        writeln!(
            self.output,
            "P\t{}\t{}\t{}\t{}\t{x}\t{y}",
            event.sim_tick, event.host_poll, event.wall_ns, event.buttons
        )?;
        Ok(())
    }

    /// T3820: `D` row, one host device event (payload encoded by `tsfp-input-intake`).
    pub fn device(&mut self, row: &Payload) -> Result<(), Error> {
        self.payload_row('D', row)
    }

    /// T3819: `S` row, a state digest; the payload starts with its own `v=<n>;` version.
    pub fn state(&mut self, row: &Payload) -> Result<(), Error> {
        if !row.payload.starts_with("v=") {
            return Err(invalid(
                "state payload must start with its own v=<n>; version",
            ));
        }
        self.payload_row('S', row)
    }

    fn payload_row(&mut self, kind: char, row: &Payload) -> Result<(), Error> {
        self.ensure_open()?;
        check_payload(&row.payload)?;
        writeln!(
            self.output,
            "{kind}\t{}\t{}\t{}\t{}",
            row.sim_tick, row.host_poll, row.wall_ns, row.payload
        )?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, Error> {
        self.ensure_open()?;
        writeln!(self.output, "E")?;
        self.output.flush()?;
        self.ended = true;
        Ok(self.output)
    }

    fn ensure_open(&self) -> Result<(), Error> {
        if self.ended {
            Err(Error::Invalid("record writer is already finished".into()))
        } else {
            Ok(())
        }
    }

    fn check_tick(&self, tick: u64) -> Result<(), Error> {
        if self.last_tick.is_some_and(|previous| tick <= previous) {
            return Err(Error::Invalid(format!(
                "simulation ticks must increase (got {tick})"
            )));
        }
        Ok(())
    }
}

/// Stream reader with strict version and truncation checks.
pub struct Reader<R> {
    input: R,
    ended: bool,
    last_tick: Option<u64>,
    version: u32,
    unlock_all: bool,
    build: Option<String>,
}

impl<R: BufRead> Reader<R> {
    pub fn new(mut input: R) -> Result<Self, Error> {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return Err(Error::Invalid("missing header".into()));
        }
        let trimmed = header.trim_end_matches(['\n', '\r']);
        // T2421: optional `saves=` and `saves-sha256=` fields bind the recording to its start storage
        let fields: Vec<&str> = trimmed
            .split('\t')
            .filter(|field| {
                !["saves=", "saves-sha256=", "build="]
                    .iter()
                    .any(|key| field.starts_with(key))
            })
            .collect();
        let build = trimmed
            .split('\t')
            .find_map(|field| field.strip_prefix("build="))
            .map(str::to_owned);
        let version = fields
            .get(1)
            .and_then(|field| field.parse::<u32>().ok())
            .filter(|version| (1..=FORMAT_VERSION).contains(version));
        let Some(version) = version.filter(|_| {
            fields.first() == Some(&"TSFP-INPUT-REC")
                && fields.get(2).and_then(|field| field.parse::<u32>().ok()) == Some(SIMULATION_HZ)
        }) else {
            return Err(Error::Invalid(format!("unsupported header {header:?}")));
        };
        let unlock_all = match (version, fields.as_slice()) {
            (3..=6, [_, _, _, "unlock-all=0"]) => false,
            (3..=6, [_, _, _, "unlock-all=1"]) => true,
            (3..=6, _) => return Err(Error::Invalid(format!("unsupported header {header:?}"))),
            (1 | 2, [_, _, _]) => false,
            _ => return Err(Error::Invalid(format!("unsupported header {header:?}"))),
        };
        Ok(Self {
            input,
            ended: false,
            last_tick: None,
            version,
            unlock_all,
            build,
        })
    }

    /// The header's format version (1 to `FORMAT_VERSION`).
    pub fn version(&self) -> u32 {
        self.version
    }

    /// T3820: the recording build (`build=` header field), `None` before v6 or when unstamped.
    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }

    /// Whether replay must enable the T2354 title-visible unlock override.
    pub fn unlock_all(&self) -> bool {
        self.unlock_all
    }

    pub fn next(&mut self) -> Result<Option<Record>, Error> {
        if self.ended {
            return Ok(None);
        }
        let mut line = String::new();
        if self.input.read_line(&mut line)? == 0 {
            return Err(Error::Invalid(
                "missing end marker (record was truncated)".into(),
            ));
        }
        let line = line.trim_end_matches(['\n', '\r']);
        if line == "E" {
            self.ended = true;
            return Ok(None);
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let record = match fields.first().copied() {
            Some("T") => {
                if fields.len() != 8 {
                    return Err(invalid("tick field count"));
                }
                let sim_tick = decimal(fields[1], "sim_tick")?;
                if self.last_tick.is_some_and(|previous| sim_tick <= previous) {
                    return Err(Error::Invalid(format!(
                        "simulation ticks are not increasing at {sim_tick}"
                    )));
                }
                let mut inputs = [None; 4];
                for (slot, field) in fields[4..].iter().enumerate() {
                    inputs[slot] = decode_input(field, slot as u8)?;
                }
                self.last_tick = Some(sim_tick);
                Record::Tick(Tick {
                    sim_tick,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    inputs,
                })
            }
            Some("G") if self.version >= 5 => {
                if fields.len() != 8 {
                    return Err(invalid("native pad field count"));
                }
                let mut inputs = [None; 4];
                for (slot, field) in fields[4..].iter().enumerate() {
                    inputs[slot] = decode_input(field, slot as u8)?;
                }
                Record::NativePad(Tick {
                    sim_tick: decimal(fields[1], "sim_tick")?,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    inputs,
                })
            }
            Some(kind @ ("D" | "S")) if self.version >= DEVICE_ROWS_VERSION => {
                if fields.len() != 5 || fields[4].is_empty() {
                    return Err(invalid("payload row field count"));
                }
                let row = Payload {
                    sim_tick: decimal(fields[1], "sim_tick")?,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    payload: fields[4].to_owned(),
                };
                if kind == "D" {
                    Record::Device(row)
                } else {
                    Record::State(row)
                }
            }
            Some("M") => {
                if fields.len() != 5 {
                    return Err(invalid("mark field count"));
                }
                Record::Mark(Mark {
                    sim_tick: decimal(fields[1], "sim_tick")?,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    label: decode_label(fields[4])?,
                })
            }
            Some("U") if self.version >= 2 => {
                let page = if self.version >= 4 {
                    if fields.len() != 8 {
                        return Err(invalid("UI event field count"));
                    }
                    u32::try_from(decimal(fields[6], "page")?)
                        .map_err(|_| invalid("page out of range"))?
                } else {
                    if fields.len() != 7 {
                        return Err(invalid("UI event field count"));
                    }
                    0
                };
                if fields.len() != if self.version >= 4 { 8 } else { 7 } {
                    return Err(invalid("UI event field count"));
                }
                Record::Ui(Ui {
                    sim_tick: decimal(fields[1], "sim_tick")?,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    context: u8::try_from(decimal(fields[4], "context")?)
                        .map_err(|_| invalid("context out of range"))?,
                    player: u8::try_from(decimal(fields[5], "player")?)
                        .map_err(|_| invalid("player out of range"))?,
                    page,
                    event: decode_ui(fields[if self.version >= 4 { 7 } else { 6 }])?,
                })
            }
            Some("P") if self.version >= 5 => {
                if fields.len() != 7 {
                    return Err(invalid("pointer field count"));
                }
                let coordinate = |text: &str, name: &str| -> Result<Option<f32>, Error> {
                    if text == "-" {
                        return Ok(None);
                    }
                    u32::from_str_radix(text, 16)
                        .map(|bits| Some(f32::from_bits(bits)))
                        .map_err(|_| Error::Invalid(format!("invalid {name}: {text:?}")))
                };
                let position = match (coordinate(fields[5], "x")?, coordinate(fields[6], "y")?) {
                    (Some(x), Some(y)) => Some((x, y)),
                    (None, None) => None,
                    _ => return Err(invalid("pointer position needs both coordinates")),
                };
                Record::Pointer(Pointer {
                    sim_tick: decimal(fields[1], "sim_tick")?,
                    host_poll: decimal(fields[2], "host_poll")?,
                    wall_ns: decimal(fields[3], "wall_ns")?,
                    buttons: u8::try_from(decimal(fields[4], "buttons")?)
                        .map_err(|_| invalid("buttons out of range"))?,
                    position,
                })
            }
            _ => return Err(invalid("unknown record line")),
        };
        Ok(Some(record))
    }
}

fn check_payload(payload: &str) -> Result<(), Error> {
    if payload.is_empty() || payload.contains(['\t', '\n', '\r']) {
        return Err(invalid("payload is empty or holds a tab or line break"));
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

fn decimal(text: &str, field: &str) -> Result<u64, Error> {
    text.parse()
        .map_err(|_| Error::Invalid(format!("invalid {field}: {text:?}")))
}

fn encode_input(input: Option<PlayerInput>) -> String {
    let Some(input) = input else {
        return "-".into();
    };
    let (source, source_id) = match input.source {
        SourceKind::Pad(id) => (1, id),
        SourceKind::MouseKeyboard => (2, 0),
        SourceKind::Script => (3, 0),
        SourceKind::Replay => (4, 0),
        SourceKind::Network { peer } => (5, u32::from(peer)),
    };
    let (look, look_x, look_y) = match input.look {
        Look::Stick(value) => (0, value.x.to_bits(), value.y.to_bits()),
        Look::Delta { yaw_deg, pitch_deg } => (1, yaw_deg.to_bits(), pitch_deg.to_bits()),
    };
    let menu = u8::from(input.menu.accept) | (u8::from(input.menu.back) << 1);
    format!(
        "{}:{}:{}:{}:{:x}:{:x}:{:x}:{:08x}:{:08x}:{}:{:08x}:{:08x}:{:08x}:{:08x}:{}:{:x}",
        input.step,
        input.slot,
        source,
        source_id,
        input.held,
        input.pressed,
        input.released,
        input.move_axis.x.to_bits(),
        input.move_axis.y.to_bits(),
        look,
        look_x,
        look_y,
        input.triggers[0].to_bits(),
        input.triggers[1].to_bits(),
        menu,
        input.assists.0
    )
}

fn decode_input(text: &str, slot: u8) -> Result<Option<PlayerInput>, Error> {
    if text == "-" {
        return Ok(None);
    }
    let fields: Vec<&str> = text.split(':').collect();
    if fields.len() != 16 {
        return Err(invalid("input field count"));
    }
    let number = |at: usize, field: &str| -> Result<u64, Error> {
        fields[at]
            .parse()
            .map_err(|_| Error::Invalid(format!("invalid input {field}: {:?}", fields[at])))
    };
    let hex = |at: usize, field: &str| -> Result<u32, Error> {
        u32::from_str_radix(fields[at], 16)
            .map_err(|_| Error::Invalid(format!("invalid input {field}: {:?}", fields[at])))
    };
    let source_id =
        u32::try_from(number(3, "source_id")?).map_err(|_| invalid("source id out of range"))?;
    let source = match number(2, "source")? {
        1 => SourceKind::Pad(source_id),
        2 if source_id == 0 => SourceKind::MouseKeyboard,
        3 if source_id == 0 => SourceKind::Script,
        4 if source_id == 0 => SourceKind::Replay,
        5 if source_id <= u32::from(u8::MAX) => SourceKind::Network {
            peer: source_id as u8,
        },
        _ => return Err(invalid("unknown input source")),
    };
    let float = |at: usize, field: &str| hex(at, field).map(f32::from_bits);
    let look_x = float(10, "look_x")?;
    let look_y = float(11, "look_y")?;
    let look = match number(9, "look")? {
        0 => Look::Stick(Vec2::new(look_x, look_y)),
        1 => Look::Delta {
            yaw_deg: look_x,
            pitch_deg: look_y,
        },
        _ => return Err(invalid("unknown look representation")),
    };
    let menu = u8::try_from(number(14, "menu")?).map_err(|_| invalid("menu flags out of range"))?;
    if menu & !3 != 0 {
        return Err(invalid("unknown menu flags"));
    }
    Ok(Some(PlayerInput {
        step: number(0, "step")?,
        slot,
        source,
        held: hex(4, "held")?,
        pressed: hex(5, "pressed")?,
        released: hex(6, "released")?,
        move_axis: Vec2::new(float(7, "move_x")?, float(8, "move_y")?),
        look,
        triggers: [float(12, "trigger_left")?, float(13, "trigger_right")?],
        menu: MenuIntent {
            accept: menu & 1 != 0,
            back: menu & 2 != 0,
        },
        assists: AssistMask(hex(15, "assists")? as u8),
    }))
}

fn decode_label(hex: &str) -> Result<String, Error> {
    if hex.len() % 2 != 0 {
        return Err(invalid("mark label has odd-length hex"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let digits =
            std::str::from_utf8(pair).map_err(|_| invalid("mark label is not ASCII hex"))?;
        bytes.push(u8::from_str_radix(digits, 16).map_err(|_| invalid("mark label is not hex"))?);
    }
    String::from_utf8(bytes).map_err(|_| invalid("mark label is not UTF-8"))
}

fn encode_ui(event: &UiEvent) -> String {
    match event {
        UiEvent::Focus(widget) => format!("F:{widget}"),
        UiEvent::Activate(widget) => format!("A:{widget}"),
        UiEvent::Back => "B".into(),
        UiEvent::Start => "S".into(),
        UiEvent::Scroll(list, delta) => format!("L:{list}:{delta}"),
        UiEvent::Hover(widget) => format!("H:{widget}"),
        UiEvent::HoverLeave => "X".into(),
        UiEvent::TextInput(ch) => format!("I:{:x}", *ch as u32),
        UiEvent::TextCommit { widget, text } => format!(
            "C:{widget}:{}",
            text.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        UiEvent::TextCancel(widget) => format!("N:{widget}"),
    }
}

fn decode_ui(text: &str) -> Result<UiEvent, Error> {
    let fields: Vec<&str> = text.split(':').collect();
    let widget = |field: &str| -> Result<u64, Error> {
        field
            .parse()
            .map_err(|_| Error::Invalid(format!("invalid UI widget id: {field:?}")))
    };
    match fields.as_slice() {
        ["F", id] => Ok(UiEvent::Focus(widget(id)?)),
        ["A", id] => Ok(UiEvent::Activate(widget(id)?)),
        ["B"] => Ok(UiEvent::Back),
        ["S"] => Ok(UiEvent::Start),
        ["L", id, delta] => Ok(UiEvent::Scroll(
            id.parse()
                .map_err(|_| Error::Invalid(format!("invalid UI list id: {id:?}")))?,
            delta
                .parse()
                .map_err(|_| Error::Invalid(format!("invalid UI scroll delta: {delta:?}")))?,
        )),
        ["H", id] => Ok(UiEvent::Hover(widget(id)?)),
        ["X"] => Ok(UiEvent::HoverLeave),
        ["I", scalar] => {
            let scalar = u32::from_str_radix(scalar, 16)
                .map_err(|_| invalid("invalid UI character scalar"))?;
            char::from_u32(scalar)
                .map(UiEvent::TextInput)
                .ok_or_else(|| invalid("invalid UI character scalar"))
        }
        ["C", id, hex] => {
            if hex.len() % 2 != 0 {
                return Err(invalid("UI text has odd-length hex"));
            }
            let mut bytes = Vec::with_capacity(hex.len() / 2);
            for pair in hex.as_bytes().chunks_exact(2) {
                let digits =
                    std::str::from_utf8(pair).map_err(|_| invalid("UI text is not ASCII hex"))?;
                bytes.push(
                    u8::from_str_radix(digits, 16).map_err(|_| invalid("UI text is not hex"))?,
                );
            }
            Ok(UiEvent::TextCommit {
                widget: widget(id)?,
                text: bytes,
            })
        }
        ["N", id] => Ok(UiEvent::TextCancel(widget(id)?)),
        _ => Err(invalid("unknown UI event")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::bits;

    fn sample() -> Tick {
        let mut inputs = [None; 4];
        inputs[0] = Some(PlayerInput {
            step: 42,
            slot: 0,
            source: SourceKind::MouseKeyboard,
            held: bits::A | bits::DPAD_UP,
            pressed: bits::A,
            released: 0,
            move_axis: Vec2::new(-0.25, 1.0),
            look: Look::Delta {
                yaw_deg: 1.125,
                pitch_deg: -2.5,
            },
            triggers: [0.0, 0.75],
            menu: MenuIntent {
                accept: true,
                back: false,
            },
            assists: AssistMask(0x15),
        });
        inputs[2] = Some(PlayerInput {
            step: 42,
            slot: 2,
            source: SourceKind::Pad(17),
            held: 0,
            pressed: 0,
            released: 0,
            move_axis: Vec2::ZERO,
            look: Look::Stick(Vec2::new(0.125, -1.0)),
            triggers: [1.0, 0.0],
            menu: MenuIntent::default(),
            assists: AssistMask::NONE,
        });
        Tick {
            sim_tick: 19,
            host_poll: 71,
            wall_ns: 123_456_789,
            inputs,
        }
    }

    #[test]
    fn round_trips_typed_inputs_and_marks_exactly() {
        let tick = sample();
        let mark = Mark {
            sim_tick: 20,
            host_poll: 73,
            wall_ns: 140_000_000,
            label: "pause – menu".into(),
        };
        let ui = Ui {
            sim_tick: 19,
            host_poll: 72,
            wall_ns: 123_456_790,
            context: 0,
            player: 0,
            page: 0x2d0be0,
            event: UiEvent::TextCommit {
                widget: 0x2d0be0,
                text: "menu – résumé".as_bytes().to_vec(),
            },
        };
        let mut writer = Writer::new(Vec::new(), true).unwrap();
        writer.tick(&tick).unwrap();
        writer.mark(&mark).unwrap();
        writer.ui(&ui).unwrap();
        let bytes = writer.finish().unwrap();
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        assert!(reader.unlock_all());
        assert_eq!(reader.next().unwrap(), Some(Record::Tick(tick)));
        assert_eq!(reader.next().unwrap(), Some(Record::Mark(mark)));
        assert_eq!(reader.next().unwrap(), Some(Record::Ui(ui)));
        assert_eq!(reader.next().unwrap(), None);
    }

    #[test]
    fn rejects_unknown_versions_bad_rows_and_truncation() {
        assert!(Reader::new(&b"TSFP-INPUT-REC\t7\t60\tunlock-all=0\n"[..]).is_err());
        let mut v1 = Reader::new(&b"TSFP-INPUT-REC\t1\t60\nE\n"[..]).unwrap();
        assert_eq!(v1.next().unwrap(), None);
        assert!(!v1.unlock_all());
        let mut v2 = Reader::new(&b"TSFP-INPUT-REC\t2\t60\nE\n"[..]).unwrap();
        assert_eq!(v2.next().unwrap(), None);
        assert!(!v2.unlock_all());
        assert!(Reader::new(&b"TSFP-INPUT-REC\t3\t60\nE\n"[..]).is_err());
        assert!(Reader::new(&b"TSFP-INPUT-REC\t3\t60\tunlock-all=yes\nE\n"[..]).is_err());
        let mut reader =
            Reader::new(&b"TSFP-INPUT-REC\t1\t60\nT\t0\t0\t0\t-\t-\t-\tbad\nE\n"[..]).unwrap();
        assert!(reader.next().is_err());
        let mut reader = Reader::new(&b"TSFP-INPUT-REC\t1\t60\n"[..]).unwrap();
        assert!(reader.next().is_err());
    }

    #[test]
    fn header_accepts_t2421_saves_fields() {
        let stamped = b"TSFP-INPUT-REC\t4\t60\tunlock-all=1\tsaves=tmp/x\tsaves-sha256=ab\nE\n";
        assert!(Reader::new(&stamped[..]).unwrap().unlock_all());
    }

    #[test]
    fn legacy_ui_rows_default_to_main_menu_and_v4_keeps_page_identity() {
        let old = b"TSFP-INPUT-REC\t3\t60\tunlock-all=0\nU\t1\t2\t3\t0\t0\tB\nE\n";
        let mut reader = Reader::new(&old[..]).unwrap();
        let Some(Record::Ui(old_ui)) = reader.next().unwrap() else {
            panic!("expected UI row")
        };
        assert_eq!(old_ui.page, 0);

        let mut writer = Writer::new(Vec::new(), false).unwrap();
        writer
            .ui(&Ui {
                sim_tick: 1,
                host_poll: 2,
                wall_ns: 3,
                context: 0,
                player: 0,
                page: 0x2d_0be0,
                event: UiEvent::Back,
            })
            .unwrap();
        let bytes = writer.finish().unwrap();
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        let Some(Record::Ui(ui)) = reader.next().unwrap() else {
            panic!("expected UI row")
        };
        assert_eq!(ui.page, 0x2d_0be0);
    }

    #[test]
    fn native_pad_rows_round_trip_beside_dash_ticks() {
        let mut writer = Writer::new(Vec::new(), false).unwrap();
        writer.native_pad(&sample()).unwrap();
        let bytes = writer.finish().unwrap();
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        assert_eq!(reader.next().unwrap(), Some(Record::NativePad(sample())));
    }

    #[test]
    fn tick_indices_must_increase() {
        let mut writer = Writer::new(Vec::new(), false).unwrap();
        writer.tick(&sample()).unwrap();
        assert!(writer.tick(&sample()).is_err());
    }

    #[test]
    fn pointer_rows_round_trip_and_older_versions_reject_them() {
        let mut writer = Writer::new(Vec::new(), false).unwrap();
        let moved = Pointer {
            sim_tick: 3,
            host_poll: 9,
            wall_ns: 11,
            buttons: BUTTON_LEFT,
            position: Some((321.5, 240.25)),
        };
        let left = Pointer {
            position: None,
            buttons: 0,
            host_poll: 10,
            ..moved
        };
        writer.pointer(&moved).unwrap();
        writer.pointer(&left).unwrap();
        let bytes = writer.finish().unwrap();
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        assert_eq!(reader.next().unwrap(), Some(Record::Pointer(moved)));
        assert_eq!(reader.next().unwrap(), Some(Record::Pointer(left)));
        assert_eq!(reader.next().unwrap(), None);
        let old = b"TSFP-INPUT-REC\t4\t60\tunlock-all=0\nP\t1\t2\t3\t0\t-\t-\nE\n";
        assert!(Reader::new(&old[..]).unwrap().next().is_err());
        let half = b"TSFP-INPUT-REC\t5\t60\tunlock-all=0\nP\t1\t2\t3\t0\t00000000\t-\nE\n";
        assert!(Reader::new(&half[..]).unwrap().next().is_err());
    }

    #[test]
    fn v6_device_and_state_rows_round_trip_with_the_build_stamp() {
        let device = Payload {
            sim_tick: 4,
            host_poll: 12,
            wall_ns: 99,
            payload: "k:KeyA:c61:P:0".into(),
        };
        let state = Payload {
            payload: "v=1;mode=3".into(),
            ..device.clone()
        };
        let mut writer = Writer::stamped(Vec::new(), false, Some("0123456789ab")).unwrap();
        writer.device(&device).unwrap();
        writer.state(&state).unwrap();
        assert!(
            writer
                .state(&Payload {
                    payload: "mode=3".into(),
                    ..device.clone()
                })
                .is_err(),
            "S payloads carry their own version"
        );
        assert!(
            writer
                .device(&Payload {
                    payload: "a\tb".into(),
                    ..device.clone()
                })
                .is_err()
        );
        let bytes = writer.finish().unwrap();
        assert!(bytes.starts_with(b"TSFP-INPUT-REC\t6\t60\tunlock-all=0\tbuild=0123456789ab\n"));
        let mut reader = Reader::new(bytes.as_slice()).unwrap();
        assert_eq!(
            (reader.version(), reader.build()),
            (6, Some("0123456789ab"))
        );
        assert_eq!(reader.next().unwrap(), Some(Record::Device(device)));
        assert_eq!(reader.next().unwrap(), Some(Record::State(state)));
        assert_eq!(reader.next().unwrap(), None);
    }

    #[test]
    fn device_and_state_rows_are_rejected_before_v6() {
        for kind in ["D", "S"] {
            let old = format!("TSFP-INPUT-REC\t5\t60\tunlock-all=0\n{kind}\t1\t2\t3\tv=1;x\nE\n");
            assert!(
                Reader::new(old.as_bytes()).unwrap().next().is_err(),
                "{kind}"
            );
        }
        let mut v5 = Reader::new(&b"TSFP-INPUT-REC\t5\t60\tunlock-all=1\nE\n"[..]).unwrap();
        assert_eq!((v5.version(), v5.build(), v5.unlock_all()), (5, None, true));
        assert_eq!(v5.next().unwrap(), None);
    }
}
