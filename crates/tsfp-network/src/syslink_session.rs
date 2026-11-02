//! System Link session lifecycle model (T2012).
//!
//! Behavioural model of the retail session machine and join handshake, from decoded instructions
//! (docs/t-net-packets.md for the 18-state table at 0x3549B8, docs/t-system-link-spec.md for the
//! rest). State numbers, branch conditions, timers and the slot/discovery tables are MEASURED;
//! the labels (`Idle`, `Lobby`, ...) are INFERRED. The model has no I/O: callers feed
//! [`StepInput`] and apply the returned [`Action`]s, which name the original callee and the
//! packet code sent. It is a specification oracle for a later transport, not a port of the
//! original functions.

use crate::syslink_codec::vdp::Control;
use crate::syslink_reliable::ReliableQueue;

/// Number of peer slots (table at 0x774180, stride 0x30C).
pub const SLOT_COUNT: usize = 16;
/// Discovery list nodes (0x665538, 8 x 0x54 bytes).
pub const DISCOVERY_NODES: usize = 8;

/// Session state `S` (`[0x76B1A4]`). The 18 table states plus the leave state written by 0x359270.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct State(pub u32);

/// Named states in the original 18-row step table plus the separately written leave state.
/// `State` remains the lossless wire/runtime value so unknown future states are not discarded;
/// callers that want a typed interpretation can use [`State::known`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum KnownState {
    Reset = 0,
    Start = 1,
    OpenInterface = 2,
    JoinBegin = 3,
    JoinWaitReply = 4,
    JoinPoll = 5,
    RosterSend = 6,
    Wait7 = 7,
    Presence = 8,
    MatchEnded = 9,
    WaitA = 0xA,
    HostLobby = 0xB,
    ClientLobby = 0xC,
    WaitD = 0xD,
    GamePending = 0xE,
    InGame = 0xF,
    PostMatch = 0x10,
    PostMatch2 = 0x11,
    Leaving = 0x12,
}

impl State {
    /// Interpret a known original state without changing the raw state value.
    pub const fn known(self) -> Option<KnownState> {
        Some(match self.0 {
            0 => KnownState::Reset,
            1 => KnownState::Start,
            2 => KnownState::OpenInterface,
            3 => KnownState::JoinBegin,
            4 => KnownState::JoinWaitReply,
            5 => KnownState::JoinPoll,
            6 => KnownState::RosterSend,
            7 => KnownState::Wait7,
            8 => KnownState::Presence,
            9 => KnownState::MatchEnded,
            0xA => KnownState::WaitA,
            0xB => KnownState::HostLobby,
            0xC => KnownState::ClientLobby,
            0xD => KnownState::WaitD,
            0xE => KnownState::GamePending,
            0xF => KnownState::InGame,
            0x10 => KnownState::PostMatch,
            0x11 => KnownState::PostMatch2,
            0x12 => KnownState::Leaving,
            _ => return None,
        })
    }
}

impl From<KnownState> for State {
    fn from(value: KnownState) -> Self {
        State(value as u32)
    }
}

/// Clock source used by a documented System Link timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockDomain {
    /// Accumulated frame delta at `[0x76B1A8]` / `[0x76B134]`.
    SessionFrameTicks,
    /// Frame counters used by the in-game input sender.
    GameFrameCounter,
    /// GetTickCount-style millisecond time used by FESL cache entries.
    MonotonicMilliseconds,
}

/// Ratio of the shared session timebase, with an explicit strict threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimebaseInterval {
    pub numerator: u8,
    pub denominator: u8,
    pub comparison: ThresholdComparison,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdComparison {
    /// The original branch uses `>`; equality does not fire the timer.
    StrictlyGreater,
    /// The original branch uses `>=`.
    AtLeast,
    /// The documented evidence gives the interval but not the equality boundary.
    Unresolved,
}

/// Timer rules recovered from T2012 §1.2, §4.1 and §6. FESL uses a different clock domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingContract {
    pub session_timer_clock: ClockDomain,
    pub host_lobby_broadcast: TimebaseInterval,
    pub client_lobby_roster: TimebaseInterval,
    pub join_request_retry: TimebaseInterval,
    pub host_snapshot: TimebaseInterval,
    pub game_input_clock: ClockDomain,
    pub fesl_cache_clock: ClockDomain,
}

/// The interval units are `[0x4E8CEC]` session timebase ticks. Its interpretation as one second
/// is xemu-level evidence; frame-tick scaling and the strict branch thresholds are instruction-derived.
pub const TIMING_CONTRACT: TimingContract = TimingContract {
    session_timer_clock: ClockDomain::SessionFrameTicks,
    host_lobby_broadcast: TimebaseInterval {
        numerator: 3,
        denominator: 2,
        comparison: ThresholdComparison::StrictlyGreater,
    },
    client_lobby_roster: TimebaseInterval {
        numerator: 2,
        denominator: 1,
        comparison: ThresholdComparison::StrictlyGreater,
    },
    join_request_retry: TimebaseInterval {
        numerator: 2,
        denominator: 1,
        comparison: ThresholdComparison::Unresolved,
    },
    host_snapshot: TimebaseInterval {
        numerator: 1,
        denominator: 1,
        comparison: ThresholdComparison::StrictlyGreater,
    },
    game_input_clock: ClockDomain::GameFrameCounter,
    fesl_cache_clock: ClockDomain::MonotonicMilliseconds,
};

impl State {
    pub const RESET: State = State(0);
    pub const START: State = State(1);
    pub const OPEN_IFACE: State = State(2);
    pub const JOIN_BEGIN: State = State(3);
    pub const JOIN_WAIT_REPLY: State = State(4);
    pub const JOIN_POLL: State = State(5);
    pub const ROSTER_SEND: State = State(6);
    pub const WAIT_7: State = State(7);
    pub const PRESENCE: State = State(8);
    pub const MATCH_ENDED: State = State(9);
    pub const WAIT_A: State = State(0xA);
    pub const LOBBY_HOST: State = State(0xB);
    pub const LOBBY_CLIENT: State = State(0xC);
    pub const WAIT_D: State = State(0xD);
    pub const GAME_PENDING: State = State(0xE);
    pub const IN_GAME: State = State(0xF);
    pub const POST_MATCH: State = State(0x10);
    pub const POST_MATCH_2: State = State(0x11);
    /// Outside the table: 0x359270 writes it while leaving; the step does nothing for it.
    pub const LEAVING: State = State(0x12);
}

/// Result of polling interface+0x60 in state 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinPoll {
    /// 0 (or any value other than 1 and 2): keep waiting.
    Waiting,
    /// 1.
    Done,
    /// 2.
    Failed,
}

/// Inputs the original reads during one `0x3544B0` step.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepInput {
    /// `[0x4E7940]`, added to the timer on entry.
    pub delta: i32,
    /// `[0x4E8CEC]`, time base (INFERRED: ticks per second).
    pub timebase: i32,
    /// Predicate 0x355EC0: role 1 or offline.
    pub is_host: bool,
    /// State 2: interface+0x3C returned nonzero.
    pub open_ok: bool,
    /// State 3: interface+0x64 is present.
    pub has_join_call: bool,
    /// State 4: `[0x76B138]` is nonzero (the join reply arrived).
    pub join_reply_seen: bool,
    pub join_poll: Option<JoinPoll>,
    /// State 0xF: `[0x7DE30C] != 9`.
    pub game_state_not_9: bool,
    /// State 0xF: frame counter `[0x76B0F0] > [0x76B0EC]`.
    pub new_frame: bool,
    /// State 0xF: frame counter `> [0x76B0F4] + 10`.
    pub frame_past_state_interval: bool,
    /// Number of local players with changed state to send as 0x0F and 0x29.
    pub changed_local_players: u8,
}

/// Side effects of a step, in execution order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Zero `0x76B100` (state 0).
    ClearScratch76b100,
    /// Call interface slot at the given offset (0x3C, 0x5C, 0x64).
    CallIface(u8),
    ClearJoinReplyFlag,
    /// Send gameplay code (via 0x354110 for 0x38, 0x3592C0 for 0x0B, 0x354150 for 0x3A, direct otherwise).
    Send(u8),
    /// State 6: send the roster block (0x353CE0) for the indexed slot.
    SendRoster,
    /// State 8: 0x356350 presence update.
    PresenceUpdate,
    /// State 0xB: 0x354240 for every one of the 16 slots (setup block, fragmented 0x2C).
    SetupAllSlots,
    /// State 0xB: helper 0x344E0 after the roster broadcast.
    Helper344e0,
    /// State 5 failure: 0x356BB0 leave with error, source line 0x599.
    LeaveWithError(u32),
}

/// The session machine: `S`, elapsed timer `T`, gameplay gate `G`, secondary timer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionMachine {
    pub state: State,
    /// `[0x76B1A8]`.
    pub timer: i32,
    /// `[0x76B110]`: nonzero lets the gameplay half of the dispatcher run.
    pub gate: bool,
    /// `[0x76B134]`, state 0xF only.
    pub secondary_timer: i32,
}

impl Default for SessionMachine {
    fn default() -> Self {
        Self {
            state: State::RESET,
            timer: 0,
            gate: false,
            secondary_timer: 0,
        }
    }
}

impl SessionMachine {
    pub fn new(state: State) -> Self {
        Self {
            state,
            ..Self::default()
        }
    }

    /// Externally written transitions (packet handlers, frontend): the original writes `S` directly.
    pub fn force_state(&mut self, state: State) {
        self.state = state;
    }

    /// Leave: 0x359270 sets S to 0x12 and sends code 0.
    pub fn leave(&mut self) -> Vec<Action> {
        self.state = State::LEAVING;
        vec![Action::Send(0x00)]
    }

    /// One step of 0x3544B0. Mirrors the table branch by branch.
    pub fn step(&mut self, input: &StepInput) -> Vec<Action> {
        self.timer = self.timer.wrapping_add(input.delta);
        let mut out = Vec::new();
        let timebase = input.timebase;
        match self.state.0 {
            0 => out.push(Action::ClearScratch76b100),
            1 => {
                self.timer = 0;
                self.gate = false;
                self.state = State::LOBBY_HOST;
            }
            2 => {
                out.push(Action::CallIface(0x3C));
                if input.open_ok {
                    self.state = State::JOIN_BEGIN;
                    self.timer = 0;
                }
                self.gate = false;
            }
            3 => {
                if input.has_join_call {
                    out.push(Action::ClearJoinReplyFlag);
                    out.push(Action::CallIface(0x64));
                    out.push(Action::Send(0x38));
                    self.state = State::JOIN_WAIT_REPLY;
                } else {
                    self.state = State::ROSTER_SEND;
                }
                self.timer = 0;
            }
            4 => {
                if input.join_reply_seen {
                    out.push(Action::CallIface(0x5C));
                    self.state = State::JOIN_POLL;
                    self.timer = 0;
                }
            }
            5 => match input.join_poll.unwrap_or(JoinPoll::Waiting) {
                JoinPoll::Done => {
                    out.push(Action::CallIface(0x64));
                    self.state = State::ROSTER_SEND;
                    self.timer = 0;
                }
                JoinPoll::Failed => {
                    out.push(Action::CallIface(0x64));
                    out.push(Action::LeaveWithError(0x599));
                }
                JoinPoll::Waiting => {}
            },
            6 => {
                self.gate = false;
                out.push(Action::SendRoster);
                self.state = State::WAIT_7;
                self.timer = 0;
            }
            7 | 9 | 0xA | 0xD => self.gate = false,
            8 => {
                self.gate = false;
                out.push(Action::PresenceUpdate);
                self.state = State::LOBBY_CLIENT;
                self.timer = 0;
            }
            0xB => {
                self.gate = false;
                if (self.timer as f32) > (timebase as f32) * 1.5 {
                    self.timer = 0;
                    out.push(Action::SetupAllSlots);
                    out.push(Action::Send(0x3A));
                    out.push(Action::Helper344e0);
                }
            }
            0xC => {
                self.gate = false;
                if self.timer > timebase.wrapping_mul(2) {
                    self.timer = 0;
                    out.push(Action::Send(0x3A));
                }
            }
            0xE => self.gate = true,
            0xF => {
                self.gate = true;
                self.secondary_timer = self.secondary_timer.wrapping_add(input.delta);
                if input.is_host && self.secondary_timer > timebase && input.game_state_not_9 {
                    self.secondary_timer = 0;
                    out.push(Action::Send(0x25));
                    out.push(Action::Send(0x43));
                }
                if input.new_frame {
                    if input.is_host {
                        out.push(Action::Send(0x0E));
                        // 0x27 / 0x28 / 0x37 depend on the game mode (not modelled): see spec.
                    } else {
                        out.push(Action::Send(0x0D));
                    }
                    for _ in 0..input.changed_local_players {
                        out.push(Action::Send(0x0F));
                        out.push(Action::Send(0x29));
                    }
                    if input.frame_past_state_interval {
                        out.push(Action::Send(0x21));
                    }
                }
            }
            0x10 => {
                if input.is_host {
                    out.push(Action::Send(0x0B));
                }
                self.timer = 0;
                self.gate = false;
                self.state = State::POST_MATCH_2;
            }
            0x11 => {
                self.timer = 0;
                self.state = if input.is_host {
                    State::LOBBY_HOST
                } else {
                    State::LOBBY_CLIENT
                };
            }
            _ => {}
        }
        out
    }
}

/// Per-peer slot (a subset of the 0x30C record).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    /// Used bit (`slot+0x34 & 1`).
    pub used: bool,
    /// Per-peer state (`slot+0x1C`): 1 joined, 3 host record on a client, 7 snapshot acknowledged.
    pub state: u32,
    /// `slot+0x20`.
    pub players: u32,
    /// Reliable sequence (`slot+0x36`).
    pub sequence: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinError {
    /// -10: the host had no free slot.
    Denied,
    /// A reply arrived that does not fit the current role.
    Unexpected,
}

/// Peer slot table plus the join handshake (receiver 0x2EBC0, sender 0x2F190).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lobby {
    pub slots: [Slot; SLOT_COUNT],
    /// `[0x774040]`: this machine's slot index.
    pub local_slot: usize,
    /// `[0x686644]`: set once the handshake completed.
    pub join_complete: bool,
    pub host: bool,
}

impl Lobby {
    /// Host side: slot 0 is the host (INFERRED: the original uses the claim helper with the local index).
    pub fn new_host(local_players: u32) -> Self {
        let mut lobby = Self {
            slots: [Slot::default(); SLOT_COUNT],
            local_slot: 0,
            join_complete: true,
            host: true,
        };
        lobby.slots[0] = Slot {
            used: true,
            state: 1,
            players: local_players,
            sequence: 0,
        };
        lobby
    }

    pub fn new_client() -> Self {
        Self {
            slots: [Slot::default(); SLOT_COUNT],
            local_slot: 0,
            join_complete: false,
            host: false,
        }
    }

    /// 0x355FE0: first free slot (INFERRED from its use; the original may reuse by identity).
    pub fn find_free_slot(&self) -> Option<usize> {
        self.slots.iter().position(|s| !s.used)
    }

    pub fn connected_count(&self) -> usize {
        self.slots.iter().filter(|s| s.used).count()
    }

    /// Host handling of -8: claim a slot, answer -9, or answer -10 when the table is full.
    pub fn host_on_join_request(&mut self, players: u32) -> Control {
        match self.find_free_slot() {
            Some(index) => {
                self.slots[index] = Slot {
                    used: true,
                    state: 1,
                    players,
                    sequence: 0,
                };
                Control::JoinAccept {
                    assigned_slot: index as u32,
                    host_slot: self.local_slot as u32,
                    host_players: self.slots[self.local_slot].players,
                }
            }
            None => Control::JoinDeny,
        }
    }

    /// Client handling of -9: record the host (state 3) and the local slot (state 1).
    pub fn client_on_join_accept(
        &mut self,
        assigned: u32,
        host: u32,
        host_players: u32,
        local_players: u32,
    ) -> Result<(), JoinError> {
        let (assigned, host) = (assigned as usize, host as usize);
        if self.host || assigned >= SLOT_COUNT || host >= SLOT_COUNT {
            return Err(JoinError::Unexpected);
        }
        self.local_slot = host;
        self.slots[host] = Slot {
            used: true,
            state: 3,
            players: host_players,
            sequence: 0,
        };
        self.local_slot = assigned;
        self.slots[assigned] = Slot {
            used: true,
            state: 1,
            players: local_players,
            sequence: 0,
        };
        self.join_complete = true;
        Ok(())
    }

    /// Client handling of -10: the original calls the leave/error routine (source line 0x30E).
    pub fn client_on_join_deny(&mut self) -> JoinError {
        self.join_complete = false;
        JoinError::Denied
    }

    /// Host handling of -11: mark the peer registered (flag 0x40 at slot+0x34) and answer -12.
    pub fn host_on_peer_register(&mut self, slot: u32) -> Result<Control, JoinError> {
        let index = slot as usize;
        if !self.host || index >= SLOT_COUNT || !self.slots[index].used {
            return Err(JoinError::Unexpected);
        }
        Ok(Control::PeerAck)
    }

    /// Release a peer slot (disconnect or -10 style error).
    pub fn release(&mut self, index: usize) {
        if index < SLOT_COUNT {
            self.slots[index] = Slot::default();
        }
    }

    /// Next reliable sequence for a slot (skips zero).
    pub fn next_sequence(&mut self, index: usize) -> u16 {
        let slot = &mut self.slots[index];
        slot.sequence = crate::syslink_codec::game::next_reliable_sequence(slot.sequence);
        slot.sequence
    }
}

/// One discovered host in the frontend list (0x54 bytes in the original).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub identity: [u8; 8],
    pub name: Vec<u8>,
    /// Node+0x40; cleared by a refusal probe (INFERRED: marks the host as no longer joinable).
    pub status: u32,
    pub free_slots: u32,
}

/// Frontend list of at most eight hosts; duplicates (same 8-byte identity) are ignored (0x2D980).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryList {
    pub nodes: Vec<DiscoveredHost>,
}

impl DiscoveryList {
    /// Query broadcast clears the list (0x2DCF0).
    pub fn reset(&mut self) {
        self.nodes.clear();
    }

    /// Insert from an advert. Returns false for a duplicate or a full pool; a duplicate does not
    /// update the existing record (MEASURED).
    pub fn insert(&mut self, identity: [u8; 8], name: &[u8], free_slots: u32) -> bool {
        if self.nodes.iter().any(|n| n.identity == identity) || self.nodes.len() >= DISCOVERY_NODES
        {
            return false;
        }
        let mut name = name.to_vec();
        name.truncate(49);
        self.nodes.push(DiscoveredHost {
            identity,
            name,
            status: 0,
            free_slots,
        });
        true
    }

    /// Probe kind 2 (refusal) from the host resets the matching node's status.
    pub fn on_refusal(&mut self, identity: [u8; 8]) {
        for node in self.nodes.iter_mut().filter(|n| n.identity == identity) {
            node.status = 0;
        }
    }
}

/// Rust-owned session state composed from the existing lifecycle model and reliable packet queue.
///
/// This is an explicit runtime owner for native callers. Existing game dispatch does not select
/// it by default; the lifted path remains the default until a later integration slice opts in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeSessionState {
    pub lifecycle: SessionMachine,
    pub reliable_packets: ReliableQueue,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> StepInput {
        StepInput {
            delta: 1,
            timebase: 60,
            ..StepInput::default()
        }
    }

    #[test]
    fn known_state_mapping_is_lossless_and_preserves_unknown_values() {
        let states = [
            KnownState::Reset,
            KnownState::Start,
            KnownState::OpenInterface,
            KnownState::JoinBegin,
            KnownState::JoinWaitReply,
            KnownState::JoinPoll,
            KnownState::RosterSend,
            KnownState::Wait7,
            KnownState::Presence,
            KnownState::MatchEnded,
            KnownState::WaitA,
            KnownState::HostLobby,
            KnownState::ClientLobby,
            KnownState::WaitD,
            KnownState::GamePending,
            KnownState::InGame,
            KnownState::PostMatch,
            KnownState::PostMatch2,
            KnownState::Leaving,
        ];
        for state in states {
            let raw = State::from(state);
            assert_eq!(raw.known(), Some(state));
            assert_eq!(state as u32, raw.0);
        }
        assert_eq!(State(0x13).known(), None);
        assert_eq!(State(0x13).0, 0x13);
    }

    #[test]
    fn timing_contract_keeps_clock_domains_and_unresolved_retry_boundary_explicit() {
        use ClockDomain::*;
        use ThresholdComparison::*;
        assert_eq!(
            TIMING_CONTRACT.host_lobby_broadcast,
            TimebaseInterval {
                numerator: 3,
                denominator: 2,
                comparison: StrictlyGreater,
            }
        );
        assert_eq!(
            TIMING_CONTRACT.client_lobby_roster,
            TimebaseInterval {
                numerator: 2,
                denominator: 1,
                comparison: StrictlyGreater,
            }
        );
        assert_eq!(TIMING_CONTRACT.join_request_retry.comparison, Unresolved);
        assert_eq!(TIMING_CONTRACT.host_snapshot.comparison, StrictlyGreater);
        assert_eq!(TIMING_CONTRACT.session_timer_clock, SessionFrameTicks);
        assert_eq!(TIMING_CONTRACT.game_input_clock, GameFrameCounter);
        assert_eq!(TIMING_CONTRACT.fesl_cache_clock, MonotonicMilliseconds);
    }

    #[test]
    fn start_goes_to_host_lobby() {
        let mut m = SessionMachine::new(State::START);
        m.timer = 99;
        assert!(m.step(&input()).is_empty());
        assert_eq!((m.state, m.timer, m.gate), (State::LOBBY_HOST, 0, false));
    }

    #[test]
    fn join_flow_states_2_to_7() {
        let mut m = SessionMachine::new(State::OPEN_IFACE);
        let mut i = input();
        assert_eq!(m.step(&i), vec![Action::CallIface(0x3C)]);
        assert_eq!(m.state, State::OPEN_IFACE);
        i.open_ok = true;
        m.step(&i);
        assert_eq!(m.state, State::JOIN_BEGIN);
        i.has_join_call = true;
        assert_eq!(
            m.step(&i),
            vec![
                Action::ClearJoinReplyFlag,
                Action::CallIface(0x64),
                Action::Send(0x38)
            ]
        );
        assert_eq!(m.state, State::JOIN_WAIT_REPLY);
        assert!(m.step(&i).is_empty());
        assert_eq!(m.state, State::JOIN_WAIT_REPLY);
        i.join_reply_seen = true;
        assert_eq!(m.step(&i), vec![Action::CallIface(0x5C)]);
        assert_eq!(m.state, State::JOIN_POLL);
        i.join_poll = Some(JoinPoll::Waiting);
        assert!(m.step(&i).is_empty());
        i.join_poll = Some(JoinPoll::Done);
        assert_eq!(m.step(&i), vec![Action::CallIface(0x64)]);
        assert_eq!(m.state, State::ROSTER_SEND);
        assert_eq!(m.step(&i), vec![Action::SendRoster]);
        assert_eq!(m.state, State::WAIT_7);
    }

    #[test]
    fn join_without_iface_call_skips_to_roster_and_failure_does_not_write_state() {
        let mut m = SessionMachine::new(State::JOIN_BEGIN);
        m.step(&input());
        assert_eq!(m.state, State::ROSTER_SEND);
        let mut m = SessionMachine::new(State::JOIN_POLL);
        let mut i = input();
        i.join_poll = Some(JoinPoll::Failed);
        assert_eq!(
            m.step(&i),
            vec![Action::CallIface(0x64), Action::LeaveWithError(0x599)]
        );
        assert_eq!(m.state, State::JOIN_POLL);
    }

    #[test]
    fn host_lobby_broadcasts_after_one_and_a_half_timebases() {
        let mut m = SessionMachine::new(State::LOBBY_HOST);
        let mut i = input();
        i.delta = 90;
        assert!(
            m.step(&i).is_empty(),
            "90 is not strictly greater than 60 * 1.5"
        );
        i.delta = 1;
        assert_eq!(
            m.step(&i),
            vec![
                Action::SetupAllSlots,
                Action::Send(0x3A),
                Action::Helper344e0
            ]
        );
        assert_eq!(m.timer, 0);
        assert_eq!(m.state, State::LOBBY_HOST);
    }

    #[test]
    fn client_lobby_resends_roster_after_two_timebases() {
        let mut m = SessionMachine::new(State::LOBBY_CLIENT);
        let mut i = input();
        i.delta = 120;
        assert!(m.step(&i).is_empty(), "strictly greater than 2 * timebase");
        i.delta = 1;
        assert_eq!(m.step(&i), vec![Action::Send(0x3A)]);
    }

    #[test]
    fn gate_is_written_by_the_documented_states_only() {
        for s in 0..=0x11u32 {
            let mut m = SessionMachine::new(State(s));
            m.gate = true;
            m.step(&input());
            let expected_open = matches!(s, 0 | 3 | 4 | 5 | 0xE | 0xF | 0x11);
            // States 0, 3, 4, 5 and 0x11 do not write the gate (it keeps its previous value).
            assert_eq!(m.gate, expected_open, "state {s:#x}");
        }
    }

    #[test]
    fn in_game_state_sends_frame_packets_by_role() {
        let mut m = SessionMachine::new(State::IN_GAME);
        let mut i = input();
        i.is_host = true;
        i.new_frame = true;
        i.changed_local_players = 1;
        i.frame_past_state_interval = true;
        i.game_state_not_9 = true;
        i.delta = 61;
        assert_eq!(
            m.step(&i),
            vec![
                Action::Send(0x25),
                Action::Send(0x43),
                Action::Send(0x0E),
                Action::Send(0x0F),
                Action::Send(0x29),
                Action::Send(0x21)
            ]
        );
        assert!(m.gate);
        i.is_host = false;
        m.secondary_timer = 0;
        i.delta = 1;
        assert_eq!(
            m.step(&i),
            vec![
                Action::Send(0x0D),
                Action::Send(0x0F),
                Action::Send(0x29),
                Action::Send(0x21)
            ]
        );
    }

    #[test]
    fn post_match_returns_to_the_role_lobby() {
        let mut m = SessionMachine::new(State::POST_MATCH);
        let mut i = input();
        i.is_host = true;
        assert_eq!(m.step(&i), vec![Action::Send(0x0B)]);
        assert_eq!(m.state, State::POST_MATCH_2);
        m.step(&i);
        assert_eq!(m.state, State::LOBBY_HOST);
        let mut m = SessionMachine::new(State::POST_MATCH_2);
        i.is_host = false;
        m.step(&i);
        assert_eq!(m.state, State::LOBBY_CLIENT);
    }

    #[test]
    fn leave_state_is_outside_the_table_and_sticks() {
        let mut m = SessionMachine::default();
        assert_eq!(m.leave(), vec![Action::Send(0)]);
        assert_eq!(m.state, State::LEAVING);
        let before = m.clone();
        let mut i = input();
        i.delta = 0;
        assert!(m.step(&i).is_empty());
        assert_eq!(m, before);
    }

    #[test]
    fn host_hands_out_slots_then_denies_when_full() {
        let mut host = Lobby::new_host(2);
        for expected in 1..SLOT_COUNT as u32 {
            match host.host_on_join_request(1) {
                Control::JoinAccept {
                    assigned_slot,
                    host_slot,
                    host_players,
                } => {
                    assert_eq!((assigned_slot, host_slot, host_players), (expected, 0, 2));
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(host.connected_count(), SLOT_COUNT);
        assert_eq!(host.host_on_join_request(1), Control::JoinDeny);
        host.release(5);
        assert!(matches!(
            host.host_on_join_request(1),
            Control::JoinAccept {
                assigned_slot: 5,
                ..
            }
        ));
    }

    #[test]
    fn full_handshake_between_host_and_client() {
        let mut host = Lobby::new_host(1);
        let mut client = Lobby::new_client();
        let Control::JoinAccept {
            assigned_slot,
            host_slot,
            host_players,
        } = host.host_on_join_request(2)
        else {
            panic!("accept expected")
        };
        client
            .client_on_join_accept(assigned_slot, host_slot, host_players, 2)
            .unwrap();
        assert!(client.join_complete);
        assert_eq!(client.local_slot, 1);
        assert_eq!(
            client.slots[0].state, 3,
            "host record is state 3 on the client"
        );
        assert_eq!(host.host_on_peer_register(1), Ok(Control::PeerAck));
        assert_eq!(host.host_on_peer_register(9), Err(JoinError::Unexpected));
        assert_eq!(client.client_on_join_deny(), JoinError::Denied);
        assert!(!client.join_complete);
    }

    #[test]
    fn discovery_list_ignores_duplicates_and_caps_at_eight() {
        let mut list = DiscoveryList::default();
        assert!(list.insert([1; 8], b"a", 15));
        assert!(
            !list.insert([1; 8], b"changed", 3),
            "duplicate does not update"
        );
        assert_eq!(list.nodes[0].name, b"a");
        for n in 2..=8u8 {
            assert!(list.insert([n; 8], b"x", 1));
        }
        assert!(!list.insert([9; 8], b"x", 1), "pool exhausted");
        list.nodes[0].status = 5;
        list.on_refusal([1; 8]);
        assert_eq!(list.nodes[0].status, 0);
        list.reset();
        assert!(list.nodes.is_empty());
    }

    #[test]
    fn reliable_sequence_per_slot_skips_zero() {
        let mut lobby = Lobby::new_host(1);
        lobby.slots[0].sequence = 0xFFFF;
        assert_eq!(lobby.next_sequence(0), 1);
        assert_eq!(lobby.next_sequence(0), 2);
    }
}
