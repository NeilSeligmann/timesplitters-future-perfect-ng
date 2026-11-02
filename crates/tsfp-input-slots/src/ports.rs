// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest controller ports 1 to 3 owned by the input router (T3771).
//!
//! The title enables the 2, 3 and 4 player rows of the Players page (0x2D2350) only for connected controllers
//! (INFERRED from code and captures, docs/t3771-two-player-pads.md). Before T3771 the router forwarded slot 0 alone and
//! the host reported one pad on port 0, so a second physical pad never reached the guest. Here the Rust-owned
//! [`PortTable`] decides which ports hold a pad and what each reports; the C multiport adapter (`--pad-ports`,
//! src/host/main.c) only applies the presence calls and reads the state at each guest poll.
//!
//! Live play: port n is present while router slot n has a device. Replay: the recording has no presence column, so a port
//! connects at the first recorded input of its slot and stays connected (old one pad recordings never connect one).
use crate::SLOTS;
use std::sync::{Mutex, OnceLock};
use tsfp_input::player::PlayerInput;
use tsfp_input::shim::{XInputPad, xinput_from_player};

/// The C `xinput_pad_state` (src/input/xinput_hle.h): packet, digital word, 8 analog bytes, LX LY RX RY.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CPadState {
    pub packet_number: u32,
    pub digital_buttons: u16,
    pub analog: [u8; 8],
    pub thumbs: [i16; 4],
}

impl From<XInputPad> for CPadState {
    fn from(pad: XInputPad) -> Self {
        Self {
            packet_number: 0,
            digital_buttons: pad.digital,
            analog: pad.analog,
            thumbs: pad.thumbs,
        }
    }
}

/// A presence change the host adapter applies (`xinput_pad_connect` / `xinput_pad_disconnect`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortChange {
    Connect(u8),
    Disconnect(u8),
}

/// Rust-owned guest port state. Port 0 is never managed here (the legacy queue pad, always present).
#[derive(Clone, Debug, Default)]
pub struct PortTable {
    present: [bool; SLOTS],
    pads: [XInputPad; SLOTS],
}

impl PortTable {
    /// One router tick. `live[n]`: router slot n has a device. `inputs[n]`: slot n's gameplay input (`None` while a native
    /// page owns input or the slot is idle, the pad then reports rest). `replaying` ignores `live` (see the module doc).
    pub fn update(
        &mut self,
        live: [bool; SLOTS],
        inputs: &[Option<PlayerInput>; SLOTS],
        replaying: bool,
    ) -> Vec<PortChange> {
        let mut changes = Vec::new();
        for port in 1..SLOTS {
            // T3781: a replay also latches a port its recording names (`live`, see `replay_presence`).
            let present = if replaying {
                self.present[port] || inputs[port].is_some() || live[port]
            } else {
                live[port]
            };
            if present != self.present[port] {
                self.present[port] = present;
                changes.push(if present {
                    PortChange::Connect(port as u8)
                } else {
                    PortChange::Disconnect(port as u8)
                });
            }
            self.pads[port] = match inputs[port] {
                Some(input) if present => xinput_from_player(&input),
                _ => XInputPad::default(),
            };
        }
        changes
    }

    /// The state port `port` reports, `None` for port 0, an absent port or an out of range port.
    pub fn state(&self, port: usize) -> Option<CPadState> {
        (port != 0 && *self.present.get(port)?).then(|| self.pads[port].into())
    }

    /// Ports 1 to 3 that hold a pad (bit n), port 0 excluded.
    pub fn present_mask(&self) -> u8 {
        (1..SLOTS)
            .filter(|&port| self.present[port])
            .fold(0, |mask, port| mask | 1 << port)
    }
}

/// T3781: recorded presence. While a native page owns input the gameplay inputs are withheld (`None`), so a pad that
/// connects on a menu page (the Players page, owner run 2026-10-26) left no trace in the recording and its replay never
/// connected port n: "2 PLAYERS" stayed greyed and the route diverged. Ports 1 to 3 that hold a device then record a
/// rest input, which the replay reads as presence.
pub fn with_presence(
    inputs: [Option<PlayerInput>; SLOTS],
    live: [bool; SLOTS],
    native_page: bool,
    step: u64,
) -> [Option<PlayerInput>; SLOTS] {
    std::array::from_fn(|port| match inputs[port] {
        None if native_page && port != 0 && live[port] => Some(rest(port as u8, step)),
        input => input,
    })
}

fn rest(slot: u8, step: u64) -> PlayerInput {
    use tsfp_input::player::{AssistMask, Look, MenuIntent, SourceKind, Vec2};
    PlayerInput {
        step,
        slot,
        source: SourceKind::Replay,
        held: 0,
        pressed: 0,
        released: 0,
        move_axis: Vec2::new(0.0, 0.0),
        look: Look::Stick(Vec2::new(0.0, 0.0)),
        triggers: [0.0; 2],
        menu: MenuIntent {
            accept: false,
            back: false,
        },
        assists: AssistMask::NONE,
    }
}

/// T3781: replay presence of a recording made before [`with_presence`]: a recording whose UI rows name player n (a
/// profile page row of player 2, a join from port n) had a pad on port n from the start. INFERRED from the recording,
/// not a measured connect time.
pub fn named_ports<'a>(
    rows: impl IntoIterator<Item = &'a tsfp_input::recording::Ui>,
) -> [bool; SLOTS] {
    use tsfp_input::recording::UiEvent;
    let mut named = [false; SLOTS];
    for row in rows {
        named[usize::from(row.player).min(SLOTS - 1)] |= row.player != 0;
        if let UiEvent::Scroll(JOIN_LIST, port) = row.event
            && let Ok(port) = usize::try_from(port)
            && (1..SLOTS).contains(&port)
        {
            named[port] = true;
        }
    }
    named
}

/// The recorded join row's list id (`tsfp_ui_input::page_intents::RECORDED_JOIN_LIST`, not a dependency here).
const JOIN_LIST: u32 = u32::MAX - 3;

static NAMED: Mutex<[bool; SLOTS]> = Mutex::new([false; SLOTS]);

/// Evidence diagnostic (not a variant): `TSFP_REPLAY_PORT_FROM=PORT:SIM_TICK[,..]` connects a named port only from
/// that replayed simulation tick on, for a recording whose pad connected mid-route (the inferred default is the start). The
/// time is the investigator's reading of the run log, labelled INFERRED wherever it is used.
fn named_from(port: usize) -> u64 {
    static FROM: std::sync::OnceLock<[u64; SLOTS]> = std::sync::OnceLock::new();
    FROM.get_or_init(|| {
        parse_port_from(&std::env::var("TSFP_REPLAY_PORT_FROM").unwrap_or_default())
    })[port]
}

fn parse_port_from(value: &str) -> [u64; SLOTS] {
    let mut from = [0; SLOTS];
    for item in value.split(',') {
        if let Some((port, step)) = item.split_once(':')
            && let (Ok(port), Ok(step)) = (port.trim().parse::<usize>(), step.trim().parse())
            && port < SLOTS
        {
            from[port] = step;
        }
    }
    from
}

/// Replay start: remember the ports the recording names ([`named_ports`]).
pub fn name_replay_ports<'a>(rows: impl IntoIterator<Item = &'a tsfp_input::recording::Ui>) {
    *NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = named_ports(rows);
}

/// Applies one presence change on the host adapter (`xinput_pad_connect` / `xinput_pad_disconnect`, wrapped by the
/// shell's C boundary); returns whether the adapter accepted it.
pub type Apply = fn(PortChange) -> bool;

static TABLE: Mutex<PortTable> = Mutex::new(PortTable {
    present: [false; SLOTS],
    pads: [XInputPad {
        digital: 0,
        analog: [0; 8],
        thumbs: [0; 4],
    }; SLOTS],
});
static HOST: OnceLock<Apply> = OnceLock::new();

fn table() -> std::sync::MutexGuard<'static, PortTable> {
    TABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Called with no Rust lock held: the adapter takes the device lock, under which the guest poll reads the table.
fn apply(host: Apply, change: PortChange) {
    let (port, verb) = match change {
        PortChange::Connect(port) => (port, "connected"),
        PortChange::Disconnect(port) => (port, "disconnected"),
    };
    let note = if host(change) {
        ""
    } else {
        ": refused by the host adapter"
    };
    eprintln!("tsfp-bevy: guest pad port {port} {verb} (router slot {port}, T3771){note}");
}

/// `--pad-ports` (src/host/main.c): the C host hands over its presence calls; ports already present are connected now.
pub fn bind(apply_change: Apply) {
    let host = *HOST.get_or_init(|| apply_change);
    let mask = table().present_mask();
    for port in (1..SLOTS as u8).filter(|port| mask & 1 << port != 0) {
        apply(host, PortChange::Connect(port));
    }
}

/// One router tick into the shared table; presence changes go to the bound host (none without `--pad-ports`).
///
/// T3781: `recorded` are the router or replay inputs before a native page withholds them (`inputs`); while replaying,
/// their presence and the players the recording names stand for the host devices (`live`).
pub fn publish(
    live: [bool; SLOTS],
    recorded: &[Option<PlayerInput>; SLOTS],
    inputs: &[Option<PlayerInput>; SLOTS],
    replay_tick: Option<u64>,
) {
    let replaying = replay_tick.is_some();
    let named = *NAMED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let live = std::array::from_fn(|port| {
        if replaying {
            let tick = replay_tick.unwrap_or(0);
            recorded[port].is_some() || (named[port] && tick >= named_from(port))
        } else {
            live[port]
        }
    });
    let changes = table().update(live, inputs, replaying);
    if let Some(host) = HOST.get() {
        for change in changes {
            apply(*host, change);
        }
    }
}

/// The state of guest port `port` for the C ports provider (guest thread, device lock held).
pub fn state(port: u32) -> Option<CPadState> {
    table().state(port as usize)
}

/// Host arguments of a router run: `--pad-ports` joins `--synthetic-pad` unless the env `TSFP_PAD_PORTS=0` or the shell
/// flag `--no-pad-ports` (removed here) opts out, or another pad backend owns the ports (`--controllers`, the C `.pad`
/// record/replay). Returns whether it was added.
pub fn host_arguments(arguments: &mut Vec<String>) -> bool {
    host_arguments_with(arguments, std::env::var("TSFP_PAD_PORTS").ok().as_deref())
}

/// [`host_arguments`] with the value of `TSFP_PAD_PORTS` given.
pub fn host_arguments_with(arguments: &mut Vec<String>, env: Option<&str>) -> bool {
    let before = arguments.len();
    arguments.retain(|argument| argument != "--no-pad-ports");
    let has = |name: &str| arguments.iter().any(|argument| argument == name);
    let add = before == arguments.len()
        && env != Some("0")
        && has("--synthetic-pad")
        && ![
            "--pad-ports",
            "--controllers",
            "--record-input",
            "--replay-input",
        ]
        .iter()
        .any(|name| has(name));
    if add {
        arguments.push("--pad-ports".into());
    }
    add
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::tests::input;
    use tsfp_input::player::bits;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn two_live_pads_report_two_ports() {
        let mut table = PortTable::default();
        let inputs = [Some(input(0, 0)), Some(input(1, bits::A)), None, None];
        let changes = table.update([true, true, false, false], &inputs, false);
        assert_eq!(changes, [PortChange::Connect(1)]);
        // Port 0 is the queue pad, always present: with port 1 the guest sees two controllers.
        assert_eq!(table.present_mask() | 1, 0b11);
        let state = table.state(1).expect("port 1 holds the second pad");
        assert_eq!(state.analog[0], 255, "A of slot 1 reaches port 1");
        assert_eq!(table.state(0), None);
        assert_eq!(table.state(2), None);
        assert_eq!(table.state(9), None);
    }

    #[test]
    fn unplugging_disconnects_and_a_native_page_reports_rest() {
        let mut table = PortTable::default();
        let pressed = [None, Some(input(1, bits::A)), None, None];
        table.update([true, true, false, false], &pressed, false);
        assert_eq!(
            table.update([true, true, false, false], &[None; SLOTS], false),
            []
        );
        assert_eq!(
            table.state(1),
            Some(CPadState::default()),
            "present at rest while a page owns input"
        );
        let changes = table.update([true, false, false, false], &pressed, false);
        assert_eq!(changes, [PortChange::Disconnect(1)]);
        assert_eq!(table.present_mask(), 0);
        assert_eq!(table.state(1), None);
    }

    #[test]
    fn replay_latches_presence_from_recorded_inputs_and_ignores_live_devices() {
        let mut table = PortTable::default();
        // T3781: while replaying the shell passes the recorded presence as `live`, never the host devices.
        assert_eq!(table.update([false; SLOTS], &[None; SLOTS], true), []);
        let recorded = [None, None, Some(input(2, 0)), None];
        assert_eq!(
            table.update([false; SLOTS], &recorded, true),
            [PortChange::Connect(2)]
        );
        assert_eq!(table.update([false; SLOTS], &[None; SLOTS], true), []);
        assert_eq!(table.present_mask(), 0b100);
    }

    /// T3781: a pad connected on a menu page leaves rest rows, which a replay reads as presence; and an older
    /// recording whose UI rows name player 2 connects port 1 from the start.
    #[test]
    fn presence_survives_native_pages_in_recordings_and_named_players_connect() {
        let live = [true, true, false, false];
        let recorded = with_presence([None; SLOTS], live, true, 7);
        assert_eq!(recorded[0], None, "port 0 is the queue pad");
        assert_eq!(
            recorded[1].map(|input| (input.slot, input.held, input.step)),
            Some((1, 0, 7))
        );
        assert_eq!(recorded[2], None);
        assert_eq!(
            with_presence([None; SLOTS], live, false, 7),
            [None; SLOTS],
            "gameplay keeps its rows"
        );
        let mut table = PortTable::default();
        let presence = std::array::from_fn(|port| recorded[port].is_some());
        assert_eq!(
            table.update(presence, &[None; SLOTS], true),
            [PortChange::Connect(1)]
        );

        use tsfp_input::recording::{Ui, UiEvent};
        let row = |player, event| Ui {
            sim_tick: 1,
            host_poll: 1,
            wall_ns: 0,
            context: 0,
            player,
            page: 0x19_4980,
            event,
        };
        assert_eq!(parse_port_from("1:430, 9:1, x"), [0, 430, 0, 0]);
        assert_eq!(named_ports(&[row(0, UiEvent::Start)]), [false; SLOTS]);
        assert_eq!(
            named_ports(&[row(1, UiEvent::Start)]),
            [false, true, false, false]
        );
        assert_eq!(
            named_ports(&[row(0, UiEvent::Scroll(JOIN_LIST, 2))]),
            [false, false, true, false]
        );
    }

    #[test]
    fn single_pad_recordings_never_connect_a_port() {
        let mut table = PortTable::default();
        let slot0 = [Some(input(0, bits::A)), None, None, None];
        for _ in 0..3 {
            assert_eq!(table.update([false; SLOTS], &slot0, true), []);
        }
        assert_eq!(table.present_mask(), 0);
    }

    static APPLIED: Mutex<Vec<PortChange>> = Mutex::new(Vec::new());

    fn record(change: PortChange) -> bool {
        APPLIED.lock().unwrap().push(change);
        true
    }

    #[test]
    fn shared_table_connects_on_bind_and_reports_state_to_the_host() {
        // The only test on the process-wide table: a pad on slot 1 before the host binds, then a press.
        publish(
            [true, true, false, false],
            &[None; SLOTS],
            &[None; SLOTS],
            None,
        );
        bind(record);
        assert_eq!(*APPLIED.lock().unwrap(), [PortChange::Connect(1)]);
        let pressed = [None, Some(input(1, bits::B)), None, None];
        publish([true, true, false, false], &pressed, &pressed, None);
        assert_eq!(state(1).map(|pad| pad.analog[1]), Some(255));
        assert_eq!(state(0), None);
        publish(
            [true, false, false, false],
            &[None; SLOTS],
            &[None; SLOTS],
            None,
        );
        assert_eq!(
            APPLIED.lock().unwrap().last(),
            Some(&PortChange::Disconnect(1))
        );
        assert_eq!(state(1), None);
    }

    #[test]
    fn c_layout_matches_xinput_pad_state() {
        assert_eq!(std::mem::size_of::<CPadState>(), 24);
        assert_eq!(std::mem::offset_of!(CPadState, digital_buttons), 4);
        assert_eq!(std::mem::offset_of!(CPadState, analog), 6);
        assert_eq!(std::mem::offset_of!(CPadState, thumbs), 14);
    }

    #[test]
    fn router_runs_add_pad_ports_unless_opted_out_or_owned_elsewhere() {
        let mut env_off = args(&["--synthetic-pad"]);
        assert!(!host_arguments_with(&mut env_off, Some("0")));
        assert!(host_arguments_with(&mut env_off, Some("1")));
        let mut plain = args(&["--synthetic-pad", "--pad-source", "gamepad"]);
        assert!(host_arguments_with(&mut plain, None));
        assert_eq!(plain.last().map(String::as_str), Some("--pad-ports"));
        assert!(!host_arguments_with(&mut plain, None), "added once");
        let mut opted_out = args(&["--synthetic-pad", "--no-pad-ports"]);
        assert!(!host_arguments_with(&mut opted_out, None));
        assert_eq!(opted_out, args(&["--synthetic-pad"]));
        for other in ["--controllers", "--record-input", "--replay-input"] {
            let mut owned = args(&["--synthetic-pad", other]);
            assert!(!host_arguments_with(&mut owned, None), "{other}");
        }
        assert!(
            !host_arguments(&mut args(&["--present", "window"])),
            "no pad, no ports"
        );
    }
}
