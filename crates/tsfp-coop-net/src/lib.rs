// SPDX-License-Identifier: GPL-3.0-or-later
//! T2153: loopback-only, bounded input exchange and reliable ordered events.
//! Contract and verification limits: docs/t2153-loopback-transport.md.
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use tsfp_input::player::{AssistMask, Look, MenuIntent, PlayerInput, SourceKind, Vec2};

pub mod census;

pub const MTU: usize = 1200;
const HEADER: usize = 13;
const WINDOW: u64 = 64;
const FRAME_BYTES: usize = 49;
const MAGIC: &[u8; 4] = b"TCN1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Malformed,
    WrongSession,
    WindowFull,
    SequenceExhausted,
    NonMonotonicInput,
}

#[derive(Debug, Default)]
pub struct Received {
    pub inputs: Vec<PlayerInput>,
    pub events: Vec<Vec<u8>>,
    pub replies: Vec<Vec<u8>>,
}

struct Pending {
    bytes: Vec<u8>,
    last_sent: Option<u64>,
}

/// Pure protocol state. Caller supplies monotonic milliseconds for retransmission;
/// simulation steps, not this clock, identify inputs. Reliable queues are bounded.
///
/// ```
/// use tsfp_coop_net::Channel;
/// let mut channel = Channel::new(7, 10);
/// assert_eq!(channel.queue_event(b"ready")?, 0);
/// assert_eq!(channel.retransmit(0).len(), 1);
/// assert!(channel.retransmit(9).is_empty());
/// assert_eq!(channel.retransmit(10).len(), 1);
/// # Ok::<(), tsfp_coop_net::Error>(())
/// ```
pub struct Channel {
    session: u64,
    history: VecDeque<PlayerInput>,
    send_next: u64,
    receive_next: u64,
    pending: BTreeMap<u64, Pending>,
    reordered: BTreeMap<u64, Vec<u8>>,
    inputs: BTreeMap<u64, PlayerInput>,
    consumed: Option<u64>,
    resend_ms: u64,
}

impl Channel {
    pub fn new(session: u64, resend_ms: u64) -> Self {
        Self {
            session,
            history: VecDeque::new(),
            send_next: 0,
            receive_next: 0,
            pending: BTreeMap::new(),
            reordered: BTreeMap::new(),
            inputs: BTreeMap::new(),
            consumed: None,
            resend_ms: resend_ms.max(1),
        }
    }

    fn header(&self, kind: u8) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(self.session.to_le_bytes());
        bytes.push(kind);
        bytes
    }

    /// Input datagrams carry this frame and the preceding two frames. Inputs older
    /// than the last consumed step cannot be applied again. No reliable stall.
    pub fn input(&mut self, input: PlayerInput) -> Result<Vec<u8>, Error> {
        validate_input(&input)?;
        if self.history.back().is_some_and(|p| p.step >= input.step) {
            return Err(Error::NonMonotonicInput);
        }
        self.history.push_back(input);
        if self.history.len() > 3 {
            self.history.pop_front();
        }
        let mut bytes = self.header(1);
        bytes.push(self.history.len() as u8);
        for p in &self.history {
            encode_input(&mut bytes, p);
        }
        Ok(bytes)
    }

    pub fn queue_event(&mut self, payload: &[u8]) -> Result<u64, Error> {
        if payload.len() > MTU - HEADER - 8 {
            return Err(Error::Malformed);
        }
        if self.pending.len() >= WINDOW as usize {
            return Err(Error::WindowFull);
        }
        let id = self.send_next;
        self.send_next = id.checked_add(1).ok_or(Error::SequenceExhausted)?;
        let mut bytes = self.header(2);
        bytes.extend(id.to_le_bytes());
        bytes.extend(payload);
        self.pending.insert(
            id,
            Pending {
                bytes,
                last_sent: None,
            },
        );
        Ok(id)
    }

    /// Pending data is retained until cumulatively acknowledged. No retry limit:
    /// the host must decide when to disconnect; queue saturation is explicit.
    pub fn retransmit(&mut self, now_ms: u64) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for packet in self.pending.values_mut() {
            if packet
                .last_sent
                .is_none_or(|t| now_ms.saturating_sub(t) >= self.resend_ms)
            {
                out.push(packet.bytes.clone());
                packet.last_sent = Some(now_ms);
            }
        }
        out
    }

    pub fn pending_events(&self) -> usize {
        self.pending.len()
    }

    /// Consume an exact frame once. Missing frames stay missing: caller explicitly
    /// chooses stall/neutral/hold-last policy and can record that decision.
    pub fn take_input(&mut self, step: u64) -> Option<PlayerInput> {
        if self.consumed.is_some_and(|old| step <= old) {
            return None;
        }
        self.consumed = Some(step);
        let result = self.inputs.remove(&step);
        self.inputs.retain(|s, _| *s > step);
        result
    }

    /// Parse completely before mutating state. Bad version/session/length/float
    /// values are rejected. Input cache and out-of-order event buffer are bounded.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<Received, Error> {
        if bytes.len() < HEADER || bytes.len() > MTU || &bytes[..4] != MAGIC {
            return Err(Error::Malformed);
        }
        if read_u64(&bytes[4..12]) != self.session {
            return Err(Error::WrongSession);
        }
        let mut result = Received::default();
        match bytes[12] {
            1 => {
                let count = *bytes.get(HEADER).ok_or(Error::Malformed)? as usize;
                if !(1..=3).contains(&count) || bytes.len() != HEADER + 1 + count * FRAME_BYTES {
                    return Err(Error::Malformed);
                }
                let mut decoded = Vec::new();
                for chunk in bytes[HEADER + 1..].chunks_exact(FRAME_BYTES) {
                    decoded.push(decode_input(chunk)?);
                }
                if decoded.windows(2).any(|p| p[0].step >= p[1].step) {
                    return Err(Error::Malformed);
                }
                for p in decoded {
                    if self.consumed.is_some_and(|s| p.step <= s)
                        || self.inputs.contains_key(&p.step)
                    {
                        continue;
                    }
                    self.inputs.insert(p.step, p);
                    result.inputs.push(p);
                }
                while self.inputs.len() > WINDOW as usize {
                    self.inputs.pop_first();
                }
            }
            2 => {
                if bytes.len() < HEADER + 8 {
                    return Err(Error::Malformed);
                }
                let id = read_u64(&bytes[HEADER..HEADER + 8]);
                if id >= self.receive_next && id - self.receive_next < WINDOW {
                    // Never advance beyond the sequence space. Sender cannot emit MAX.
                    if id == u64::MAX {
                        return Err(Error::SequenceExhausted);
                    }
                    self.reordered
                        .entry(id)
                        .or_insert_with(|| bytes[HEADER + 8..].to_vec());
                    while let Some(event) = self.reordered.remove(&self.receive_next) {
                        result.events.push(event);
                        self.receive_next += 1;
                    }
                }
                let mut ack = self.header(3);
                ack.extend(self.receive_next.to_le_bytes());
                result.replies.push(ack);
            }
            3 => {
                if bytes.len() != HEADER + 8 {
                    return Err(Error::Malformed);
                }
                let next = read_u64(&bytes[HEADER..]);
                if next > self.send_next {
                    return Err(Error::Malformed);
                }
                self.pending.retain(|id, _| *id >= next);
            }
            _ => return Err(Error::Malformed),
        }
        Ok(result)
    }
}

fn read_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[..8].try_into().unwrap())
}
fn validate_input(p: &PlayerInput) -> Result<(), Error> {
    let (look, stick) = match p.look {
        Look::Stick(v) => ([v.x, v.y], true),
        Look::Delta { yaw_deg, pitch_deg } => ([yaw_deg, pitch_deg], false),
    };
    if p.slot > 3
        || p.held > 0xffff
        || p.pressed > 0xffff
        || p.released > 0xffff
        || p.assists.0 > 0x1f
        || [p.move_axis.x, p.move_axis.y]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1.0)
        || look
            .iter()
            .any(|v| !v.is_finite() || (stick && v.abs() > 1.0))
        || p.triggers
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err(Error::Malformed);
    }
    Ok(())
}
fn encode_input(bytes: &mut Vec<u8>, p: &PlayerInput) {
    bytes.extend(p.step.to_le_bytes());
    bytes.push(p.slot);
    for word in [p.held, p.pressed, p.released] {
        bytes.extend(word.to_le_bytes());
    }
    let (kind, look) = match p.look {
        Look::Stick(v) => (0, [v.x, v.y]),
        Look::Delta { yaw_deg, pitch_deg } => (1, [yaw_deg, pitch_deg]),
    };
    bytes.push(kind);
    for f in [
        p.move_axis.x,
        p.move_axis.y,
        look[0],
        look[1],
        p.triggers[0],
        p.triggers[1],
    ] {
        bytes.extend(f.to_le_bytes());
    }
    bytes.push(u8::from(p.menu.accept) | (u8::from(p.menu.back) << 1));
    bytes.push(p.assists.0);
    bytes.push(0); // Reserved; must be zero for this protocol version.
}
fn decode_input(b: &[u8]) -> Result<PlayerInput, Error> {
    let word = |offset| u32::from_le_bytes(b[offset..offset + 4].try_into().unwrap());
    let float = |offset| f32::from_bits(word(offset));
    let look = match b[21] {
        0 => Look::Stick(Vec2::new(float(30), float(34))),
        1 => Look::Delta {
            yaw_deg: float(30),
            pitch_deg: float(34),
        },
        _ => return Err(Error::Malformed),
    };
    if b[46] > 3 || b[48] != 0 {
        return Err(Error::Malformed);
    }
    let p = PlayerInput {
        step: read_u64(b),
        slot: b[8],
        source: SourceKind::Network { peer: 0 },
        held: word(9),
        pressed: word(13),
        released: word(17),
        move_axis: Vec2::new(float(22), float(26)),
        look,
        triggers: [float(38), float(42)],
        menu: MenuIntent {
            accept: b[46] & 1 != 0,
            back: b[46] & 2 != 0,
        },
        assists: AssistMask(b[47]),
    };
    validate_input(&p)?;
    Ok(p)
}

/// Connected UDP endpoint. Stage 0 deliberately refuses non-loopback peers;
/// session identity is a demultiplexer, not authentication.
pub struct Loopback {
    socket: UdpSocket,
}
impl Loopback {
    pub fn bind() -> io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0")?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    pub fn connect(&self, peer: SocketAddr) -> io::Result<()> {
        if !peer.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "loopback peer required",
            ));
        }
        self.socket.connect(peer)
    }
    pub fn send(&self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MTU {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "MTU exceeded"));
        }
        self.socket.send(bytes).map(|_| ())
    }
    /// Read at most `budget` datagrams so a frame loop cannot be starved.
    /// Oversize packets are rejected rather than accepted after truncation.
    pub fn poll(
        &self,
        channel: &mut Channel,
        budget: usize,
    ) -> io::Result<Vec<Result<Received, Error>>> {
        let mut out = Vec::new();
        let mut bytes = [0; MTU + 1];
        for _ in 0..budget {
            match self.socket.recv(&mut bytes) {
                Ok(size) => out.push(channel.receive(&bytes[..size])),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}
