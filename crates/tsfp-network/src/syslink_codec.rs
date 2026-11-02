//! System Link packet codec (T2012).
//!
//! Typed encode/decode for the packet layers of the retail System Link stack, derived from decoded
//! original instructions (see `docs/t-system-link-spec.md`, `docs/t-net-packets.md` and
//! `docs/t-syslink-protocol.md`). Every field carries a confidence label in [`FIELD_CATALOGUE`]:
//! `Measured` means the width/offset/order comes from decoded instructions, `Inferred` means the
//! meaning (or an unproven length) is interpretation. Nothing here was checked against live traffic
//! (see the spec for the xemu attempt). All multi-byte values are little endian, there is no network
//! byte order conversion in the game payloads.
//!
//! Layers:
//! 1. [`discovery`]: bare UDP datagrams, port 1001 broadcast, `u32` discriminator first.
//! 2. [`vdp`]: the session transport frame (`u16` header length, header, optional voice tail) used
//!    on the protocol 0xFE (XNet VDP) socket bound to port 1000, and its control headers.
//! 3. [`game`]: the gameplay packet `code:u8, key:u8, sequence:u16, payload`, carried inside a
//!    `0xDEADB33F` control header, plus the 0x2E fragmentation envelope.
//! 4. [`fesl`]: the EA "FESL" LAN advertisement (`gEA` magic, 384 bytes, port 9999).

use std::fmt;

/// Confidence label of a field or layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    /// Width, offset and order decoded from original instructions.
    Measured,
    /// Interpretation (meaning, name, or an unproven bound).
    Inferred,
}

/// One documented field of a packet layout.
#[derive(Clone, Copy, Debug)]
pub struct FieldSpec {
    pub packet: &'static str,
    pub field: &'static str,
    /// Offset from the start of the packet body (after the framing named by the layer), `None` if variable.
    pub offset: Option<u16>,
    pub width: &'static str,
    pub confidence: Confidence,
}

macro_rules! f {
    ($p:expr, $n:expr, $o:expr, $w:expr, $c:ident) => {
        FieldSpec {
            packet: $p,
            field: $n,
            offset: $o,
            width: $w,
            confidence: Confidence::$c,
        }
    };
}

/// Field catalogue for the fixed-layout packets implemented below (sources in the spec).
pub const FIELD_CATALOGUE: &[FieldSpec] = &[
    f!("vdp_frame", "header_len", Some(0), "u16", Measured),
    f!("vdp_frame", "header", Some(2), "header_len bytes", Measured),
    f!(
        "vdp_frame",
        "voice_tail",
        None,
        "rest of datagram",
        Measured
    ),
    f!("control", "discriminator", Some(0), "u32", Measured),
    f!(
        "join_request(-8)",
        "player_count",
        Some(12),
        "u32",
        Measured
    ),
    f!(
        "join_request(-8)",
        "user_tag",
        Some(16),
        "u32 (-1 = none)",
        Measured
    ),
    f!(
        "join_request(-8)",
        "user_name",
        Some(20),
        "32 bytes NUL terminated",
        Measured
    ),
    f!("join_accept(-9)", "assigned_slot", Some(4), "u32", Measured),
    f!("join_accept(-9)", "host_slot", Some(8), "u32", Measured),
    f!(
        "join_accept(-9)",
        "host_player_count",
        Some(12),
        "u32",
        Measured
    ),
    f!(
        "join_deny(-10)",
        "padding",
        Some(4),
        "48 bytes, uninitialised on the wire",
        Inferred
    ),
    f!("peer_register(-11)", "slot", Some(4), "u32", Measured),
    f!(
        "peer_register(-11)",
        "user_tag",
        Some(16),
        "u32 (-1 = none)",
        Measured
    ),
    f!(
        "peer_register(-11)",
        "user_name",
        Some(20),
        "32 bytes",
        Measured
    ),
    f!("peer_ack(-12)", "padding", Some(4), "48 bytes", Inferred),
    f!(
        "game_data(0xDEADB33F)",
        "game_packet",
        Some(4),
        "code,key,seq16,payload",
        Measured
    ),
    f!("game_header", "code", Some(0), "u8", Measured),
    f!(
        "game_header",
        "key",
        Some(1),
        "u8 (low nibble session byte xor payload byte, high nibble selector)",
        Inferred
    ),
    f!(
        "game_header",
        "sequence",
        Some(2),
        "u16 (reliable only, never 0)",
        Measured
    ),
    f!("fragment(0x2e)", "message_seq", Some(4), "u32", Measured),
    f!("fragment(0x2e)", "inner_code", Some(8), "u32", Measured),
    f!("fragment(0x2e)", "chunk_len", Some(12), "u16", Measured),
    f!("fragment(0x2e)", "chunk_index", Some(14), "u8", Measured),
    f!("fragment(0x2e)", "chunk_total", Some(15), "u8", Measured),
    f!(
        "fragment(0x2e)",
        "chunk",
        Some(16),
        "chunk_len bytes (max 239)",
        Measured
    ),
    f!(
        "discovery_query(-5)",
        "discriminator",
        Some(0),
        "u32 = 0xFFFFFFFB",
        Measured
    ),
    f!(
        "discovery_advert(-6)",
        "game_a",
        Some(4),
        "u32 (byte 0x78adae)",
        Inferred
    ),
    f!(
        "discovery_advert(-6)",
        "game_b",
        Some(8),
        "u32 (byte 0x78ad5b)",
        Inferred
    ),
    f!(
        "discovery_advert(-6)",
        "game_c",
        Some(12),
        "i32 (sign-extended byte 0x78adac)",
        Inferred
    ),
    f!(
        "discovery_advert(-6)",
        "free_slots",
        Some(16),
        "u32 = 16 - connected",
        Inferred
    ),
    f!(
        "discovery_advert(-6)",
        "name",
        Some(20),
        "30 bytes",
        Measured
    ),
    f!(
        "discovery_advert(-6)",
        "random_id",
        Some(50),
        "8 bytes",
        Measured
    ),
    f!(
        "discovery_advert(-6)",
        "xnaddr",
        Some(58),
        "36 bytes",
        Measured
    ),
    f!(
        "discovery_advert(-6)",
        "session_id",
        Some(94),
        "8 bytes",
        Measured
    ),
    f!(
        "discovery_advert(-6)",
        "key",
        Some(102),
        "16 bytes (last 2 transmitted bytes unassigned)",
        Measured
    ),
    f!("discovery_probe(0xABCD0123)", "a", Some(4), "u32", Measured),
    f!("discovery_probe(0xABCD0123)", "b", Some(8), "u32", Measured),
    f!(
        "discovery_probe(0xABCD0123)",
        "session_id",
        Some(12),
        "8 bytes",
        Measured
    ),
    f!(
        "discovery_probe(0xABCD0123)",
        "kind",
        Some(20),
        "u8 (1 = join intent, 2 = refusal)",
        Inferred
    ),
    f!("fesl_lan(gEA)", "magic", Some(0), "'g','E','A'", Measured),
    f!(
        "fesl_lan(gEA)",
        "interval",
        Some(3),
        "u8 seconds, clamp 2..250",
        Measured
    ),
    f!("fesl_lan(gEA)", "word", Some(4), "u32 big endian", Measured),
    f!(
        "fesl_lan(gEA)",
        "strings",
        Some(8),
        "32+32+192+120 NUL terminated",
        Measured
    ),
];

/// Decode failure. The original performs no equivalent checks (see the spec, hostile packets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodecError {
    Truncated { needed: usize, have: usize },
    BadDiscriminator(u32),
    BadMagic,
    BadLength(&'static str),
    BadFragment(&'static str),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CodecError {}

pub type Result<T> = std::result::Result<T, CodecError>;

/// Little endian cursor reader.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let out = &self.data[self.pos..];
        self.pos = self.data.len();
        out
    }
    pub fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        if self.remaining() < count {
            return Err(CodecError::Truncated {
                needed: count,
                have: self.remaining(),
            });
        }
        let out = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(out)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.bytes(2)?.try_into().expect("2 bytes"),
        ))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?.try_into().expect("4 bytes"),
        ))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    /// A float stored as its top three bytes (`bits >> 8`), see [`f32_to_f24`].
    pub fn f24(&mut self) -> Result<F24> {
        let b = self.bytes(3)?;
        Ok(F24(u32::from_le_bytes([b[0], b[1], b[2], 0])))
    }
    pub fn v24(&mut self) -> Result<[F24; 3]> {
        Ok([self.f24()?, self.f24()?, self.f24()?])
    }
}

/// Little endian writer.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    fn little_endian<const N: usize>(&mut self, bytes: [u8; N]) -> &mut Self {
        self.buf.extend_from_slice(&bytes);
        self
    }

    pub fn new() -> Self {
        Self::default()
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.little_endian(v.to_le_bytes())
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.little_endian(v.to_le_bytes())
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.u32(v as u32)
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(v);
        self
    }
    pub fn zeros(&mut self, count: usize) -> &mut Self {
        self.buf.resize(self.buf.len() + count, 0);
        self
    }
    pub fn f24(&mut self, v: F24) -> &mut Self {
        self.buf.extend_from_slice(&v.0.to_le_bytes()[..3]);
        self
    }
    pub fn v24(&mut self, v: [F24; 3]) -> &mut Self {
        for c in v {
            self.f24(c);
        }
        self
    }
}

/// The 24-bit float of the game packets: the high three bytes of an IEEE binary32, stored little
/// endian (`bits >> 8`). The low mantissa byte is lost. Holds the 24-bit value, not the float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct F24(pub u32);

pub fn f32_to_f24(value: f32) -> F24 {
    F24(value.to_bits() >> 8)
}

pub fn f24_to_f32(value: F24) -> f32 {
    f32::from_bits(value.0 << 8)
}

/// Object reference (writer 0x359400, reader 0x3594b0). Not a total codec in the original: an
/// object with no encodable kind writes nothing, modelled here by the caller not writing a `Ref`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectRef {
    Null,
    /// Tag 1: player-record index.
    Player(u8),
    /// Tag 2: 32-bit object id resolved through the pool lookup 0x359390.
    Id(u32),
    /// Tag 3: ordinal in linked list 0x7a20c0 (0xFF = not found).
    Linked(u8),
}

impl ObjectRef {
    pub fn encode(self, w: &mut Writer) {
        match self {
            ObjectRef::Null => {
                w.u8(0);
            }
            ObjectRef::Player(i) => {
                w.u8(1).u8(i);
            }
            ObjectRef::Id(id) => {
                w.u8(2).u32(id);
            }
            ObjectRef::Linked(i) => {
                w.u8(3).u8(i);
            }
        }
    }
    /// Unknown tags consume the tag and yield `Null`, as the original does.
    pub fn decode(r: &mut Reader<'_>) -> Result<Self> {
        Ok(match r.u8()? {
            1 => ObjectRef::Player(r.u8()?),
            2 => ObjectRef::Id(r.u32()?),
            3 => ObjectRef::Linked(r.u8()?),
            _ => ObjectRef::Null,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Layer 1: discovery datagrams (bare UDP, port 1001, destination 255.255.255.255 for queries)
// ---------------------------------------------------------------------------------------------
pub mod discovery {
    use super::*;

    pub const PORT: u16 = 1001;
    pub const QUERY: u32 = 0xFFFF_FFFB;
    pub const ADVERT: u32 = 0xFFFF_FFFA;
    pub const PROBE: u32 = 0xABCD_0123;
    pub const PROBE_ACK: u32 = 0xFFFF_FFFE;
    pub const ADVERT_LEN: usize = 120;
    pub const PROBE_LEN: usize = 24;
    pub const PROBE_ACK_LEN: usize = 72;

    /// 120-byte host advertisement (header -6).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Advert {
        pub game_a: u32,
        pub game_b: u32,
        pub game_c: i32,
        pub free_slots: u32,
        /// Session name, at most 29 bytes plus NUL in the 30-byte field.
        pub name: Vec<u8>,
        pub random_id: [u8; 8],
        pub xnaddr: [u8; 36],
        pub session_id: [u8; 8],
        pub key: [u8; 16],
    }

    /// Join intent / refusal probe (header 0xABCD0123), 24 bytes.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Probe {
        pub a: u32,
        pub b: u32,
        pub session_id: [u8; 8],
        pub kind: u8,
    }

    /// Host answer to a probe (header -2), 72 bytes: two echoed words, 60 opaque bytes.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ProbeAck {
        pub a: u32,
        pub b: u32,
        pub opaque: [u8; 60],
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Datagram {
        Query,
        Advert(Advert),
        Probe(Probe),
        ProbeAck(ProbeAck),
    }

    pub fn encode(d: &Datagram) -> Vec<u8> {
        let mut w = Writer::new();
        match d {
            Datagram::Query => {
                w.u32(QUERY);
            }
            Datagram::Advert(a) => {
                w.u32(ADVERT)
                    .u32(a.game_a)
                    .u32(a.game_b)
                    .i32(a.game_c)
                    .u32(a.free_slots);
                let mut name = [0u8; 30];
                let n = a.name.len().min(29);
                name[..n].copy_from_slice(&a.name[..n]);
                w.bytes(&name)
                    .bytes(&a.random_id)
                    .bytes(&a.xnaddr)
                    .bytes(&a.session_id)
                    .bytes(&a.key);
                w.zeros(ADVERT_LEN - w.buf.len());
            }
            Datagram::Probe(p) => {
                w.u32(PROBE)
                    .u32(p.a)
                    .u32(p.b)
                    .bytes(&p.session_id)
                    .u8(p.kind)
                    .zeros(3);
            }
            Datagram::ProbeAck(p) => {
                w.u32(PROBE_ACK).u32(p.a).u32(p.b).bytes(&p.opaque);
            }
        }
        w.buf
    }

    /// Decode one datagram. The original only requires length >= 4 before dispatch; here the
    /// per-type minimum length is enforced (a deliberate improvement, see the spec).
    pub fn decode(data: &[u8]) -> Result<Datagram> {
        let mut r = Reader::new(data);
        let header = r.u32()?;
        match header {
            QUERY => Ok(Datagram::Query),
            ADVERT => {
                if data.len() < ADVERT_LEN {
                    return Err(CodecError::BadLength("advert < 120"));
                }
                let game_a = r.u32()?;
                let game_b = r.u32()?;
                let game_c = r.i32()?;
                let free_slots = r.u32()?;
                let raw = r.bytes(30)?;
                let end = raw.iter().position(|&b| b == 0).unwrap_or(30);
                Ok(Datagram::Advert(Advert {
                    game_a,
                    game_b,
                    game_c,
                    free_slots,
                    name: raw[..end].to_vec(),
                    random_id: r.bytes(8)?.try_into().expect("8"),
                    xnaddr: r.bytes(36)?.try_into().expect("36"),
                    session_id: r.bytes(8)?.try_into().expect("8"),
                    key: r.bytes(16)?.try_into().expect("16"),
                }))
            }
            PROBE => {
                if data.len() < PROBE_LEN {
                    return Err(CodecError::BadLength("probe < 24"));
                }
                Ok(Datagram::Probe(Probe {
                    a: r.u32()?,
                    b: r.u32()?,
                    session_id: r.bytes(8)?.try_into().expect("8"),
                    kind: r.u8()?,
                }))
            }
            PROBE_ACK => {
                if data.len() < PROBE_ACK_LEN {
                    return Err(CodecError::BadLength("probe ack < 72"));
                }
                Ok(Datagram::ProbeAck(ProbeAck {
                    a: r.u32()?,
                    b: r.u32()?,
                    opaque: r.bytes(60)?.try_into().expect("60"),
                }))
            }
            other => Err(CodecError::BadDiscriminator(other)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Layer 2: session transport frame and control headers (protocol 0xFE socket, port 1000)
// ---------------------------------------------------------------------------------------------
pub mod vdp {
    use super::*;

    pub const PORT: u16 = 1000;
    /// IP protocol number of the session socket (`socket(2, 2, 0xFE)`); equals XNet IPPROTO_VDP (INFERRED).
    pub const IP_PROTOCOL: u8 = 0xFE;
    /// Receive buffer of the original (0x2EA20 reads at most 0x200 bytes).
    pub const MAX_DATAGRAM: usize = 0x200;
    /// Header copy limit of the control receiver (0x2EBC0 passes 0x12C).
    pub const MAX_HEADER: usize = 0x12C;

    pub const JOIN_REQUEST: u32 = 0xFFFF_FFF8; // -8
    pub const JOIN_ACCEPT: u32 = 0xFFFF_FFF7; // -9
    pub const JOIN_DENY: u32 = 0xFFFF_FFF6; // -10
    pub const PEER_REGISTER: u32 = 0xFFFF_FFF5; // -11
    pub const PEER_ACK: u32 = 0xFFFF_FFF4; // -12
    pub const PING_REQUEST: u32 = 0x9852_5928;
    pub const PING_REPLY: u32 = 0x1111_2234;
    pub const GAME_DATA: u32 = 0xDEAD_B33F;
    /// Header length of every fixed control message (0x34).
    pub const CONTROL_LEN: usize = 0x34;
    pub const USER_NAME_LEN: usize = 32;

    /// The wire discriminator for each control-message family. The names describe the
    /// instruction-derived dispatch cases; the -11 sender remains unresolved (T2012 §9).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u32)]
    pub enum ControlKind {
        JoinRequest = JOIN_REQUEST,
        JoinAccept = JOIN_ACCEPT,
        JoinDeny = JOIN_DENY,
        PeerRegister = PEER_REGISTER,
        PeerAck = PEER_ACK,
        PingRequest = PING_REQUEST,
        PingReply = PING_REPLY,
        GameData = GAME_DATA,
    }

    impl TryFrom<u32> for ControlKind {
        type Error = CodecError;

        fn try_from(value: u32) -> Result<Self> {
            match value {
                JOIN_REQUEST => Ok(Self::JoinRequest),
                JOIN_ACCEPT => Ok(Self::JoinAccept),
                JOIN_DENY => Ok(Self::JoinDeny),
                PEER_REGISTER => Ok(Self::PeerRegister),
                PEER_ACK => Ok(Self::PeerAck),
                PING_REQUEST => Ok(Self::PingRequest),
                PING_REPLY => Ok(Self::PingReply),
                GAME_DATA => Ok(Self::GameData),
                other => Err(CodecError::BadDiscriminator(other)),
            }
        }
    }

    /// `u16 header_len, header, tail` (sender 0x2E980, receiver 0x2EA20).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Frame {
        pub header: Vec<u8>,
        /// Voice tail (consumed by 0x2FDE0 as 11-byte records, INFERRED), may be empty.
        pub tail: Vec<u8>,
    }

    impl Frame {
        /// Encode the frame without truncating an unrepresentable `u16` header extent.
        pub fn encode(&self) -> Result<Vec<u8>> {
            let header_len = u16::try_from(self.header.len())
                .map_err(|_| CodecError::BadLength("frame header exceeds u16 extent"))?;
            let mut w = Writer::new();
            w.u16(header_len).bytes(&self.header).bytes(&self.tail);
            Ok(w.buf)
        }
        pub fn decode(datagram: &[u8]) -> Result<Frame> {
            if datagram.len() < 4 || datagram.len() > MAX_DATAGRAM {
                return Err(CodecError::BadLength("frame outside 4..=0x200"));
            }
            let mut r = Reader::new(datagram);
            let len = r.u16()? as usize;
            let header = r.bytes(len)?.to_vec();
            Ok(Frame {
                header,
                tail: r.rest().to_vec(),
            })
        }
    }

    /// Control messages (first `u32` of the frame header).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Control {
        /// -8, sent by 0x2F190 to the host: joiner player count, optional user tag and name.
        JoinRequest {
            players: u32,
            user_tag: i32,
            user_name: Vec<u8>,
        },
        /// -9 reply: the slot the host gave the joiner, the host's own slot and player count.
        JoinAccept {
            assigned_slot: u32,
            host_slot: u32,
            host_players: u32,
        },
        /// -10 reply on slot exhaustion (peer calls the leave/error routine 0x356BB0).
        JoinDeny,
        /// -11, registers a peer's identity at a slot (INFERRED role).
        PeerRegister {
            slot: u32,
            user_tag: i32,
            user_name: Vec<u8>,
        },
        /// -12 reply to -11.
        PeerAck,
        /// 0x98525928, asks for a 4-byte reply.
        PingRequest,
        /// 0x11112234 reply (4 bytes).
        PingReply,
        /// 0xDEADB33F + gameplay packet bytes (header length = packet length + 4).
        GameData(Vec<u8>),
    }

    fn name_field(name: &[u8]) -> [u8; USER_NAME_LEN] {
        let mut out = [0u8; USER_NAME_LEN];
        let n = name.len().min(USER_NAME_LEN - 1);
        out[..n].copy_from_slice(&name[..n]);
        out
    }

    fn name_from(raw: &[u8]) -> Vec<u8> {
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        raw[..end].to_vec()
    }

    impl Control {
        pub fn kind(&self) -> ControlKind {
            match self {
                Control::JoinRequest { .. } => ControlKind::JoinRequest,
                Control::JoinAccept { .. } => ControlKind::JoinAccept,
                Control::JoinDeny => ControlKind::JoinDeny,
                Control::PeerRegister { .. } => ControlKind::PeerRegister,
                Control::PeerAck => ControlKind::PeerAck,
                Control::PingRequest => ControlKind::PingRequest,
                Control::PingReply => ControlKind::PingReply,
                Control::GameData(_) => ControlKind::GameData,
            }
        }

        /// Encode to the frame header bytes (no `u16` length prefix).
        pub fn encode_header(&self) -> Vec<u8> {
            let mut w = Writer::new();
            match self {
                Control::JoinRequest {
                    players,
                    user_tag,
                    user_name,
                } => {
                    w.u32(JOIN_REQUEST)
                        .u32(0)
                        .u32(0)
                        .u32(*players)
                        .i32(*user_tag);
                    w.bytes(&name_field(user_name));
                }
                Control::JoinAccept {
                    assigned_slot,
                    host_slot,
                    host_players,
                } => {
                    w.u32(JOIN_ACCEPT)
                        .u32(*assigned_slot)
                        .u32(*host_slot)
                        .u32(*host_players);
                    w.zeros(CONTROL_LEN - w.buf.len());
                }
                Control::JoinDeny => {
                    w.u32(JOIN_DENY).zeros(CONTROL_LEN - 4);
                }
                Control::PeerRegister {
                    slot,
                    user_tag,
                    user_name,
                } => {
                    w.u32(PEER_REGISTER).u32(*slot).u32(0).u32(0).i32(*user_tag);
                    w.bytes(&name_field(user_name));
                }
                Control::PeerAck => {
                    w.u32(PEER_ACK).zeros(CONTROL_LEN - 4);
                }
                Control::PingRequest => {
                    w.u32(PING_REQUEST);
                }
                Control::PingReply => {
                    w.u32(PING_REPLY);
                }
                Control::GameData(packet) => {
                    w.u32(GAME_DATA).bytes(packet);
                }
            }
            w.buf
        }

        pub fn decode_header(header: &[u8]) -> Result<Control> {
            let mut r = Reader::new(header);
            let disc = r.u32()?;
            let need = |n: usize| {
                if header.len() < n {
                    Err(CodecError::BadLength(
                        "control shorter than its documented extent",
                    ))
                } else {
                    Ok(())
                }
            };
            match disc {
                JOIN_REQUEST => {
                    need(CONTROL_LEN)?;
                    r.bytes(8)?;
                    let players = r.u32()?;
                    let user_tag = r.i32()?;
                    Ok(Control::JoinRequest {
                        players,
                        user_tag,
                        user_name: name_from(r.bytes(USER_NAME_LEN)?),
                    })
                }
                JOIN_ACCEPT => {
                    need(CONTROL_LEN)?;
                    Ok(Control::JoinAccept {
                        assigned_slot: r.u32()?,
                        host_slot: r.u32()?,
                        host_players: r.u32()?,
                    })
                }
                JOIN_DENY => {
                    need(CONTROL_LEN)?;
                    Ok(Control::JoinDeny)
                }
                PEER_REGISTER => {
                    need(CONTROL_LEN)?;
                    let slot = r.u32()?;
                    r.bytes(8)?;
                    let user_tag = r.i32()?;
                    Ok(Control::PeerRegister {
                        slot,
                        user_tag,
                        user_name: name_from(r.bytes(USER_NAME_LEN)?),
                    })
                }
                PEER_ACK => {
                    need(CONTROL_LEN)?;
                    Ok(Control::PeerAck)
                }
                PING_REQUEST => Ok(Control::PingRequest),
                PING_REPLY => Ok(Control::PingReply),
                GAME_DATA => Ok(Control::GameData(r.rest().to_vec())),
                other => Err(CodecError::BadDiscriminator(other)),
            }
        }

        /// Whole datagram (`u16` length, header, no tail).
        pub fn to_datagram(&self) -> Result<Vec<u8>> {
            Frame {
                header: self.encode_header(),
                tail: Vec::new(),
            }
            .encode()
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Layer 3: gameplay packets
// ---------------------------------------------------------------------------------------------
pub mod game {
    use super::*;

    /// Chunk payload limit of the 0x2E fragmenter (0x353FE0).
    pub const FRAGMENT_CHUNK: usize = 0xEF;

    /// Gameplay packet codes with an implemented typed layout (names INFERRED, codes MEASURED).
    pub mod code {
        pub const LEAVE: u8 = 0x00;
        pub const SESSION_COUNTER: u8 = 0x01;
        pub const SESSION_EVENT: u8 = 0x07;
        pub const SLOT_TYPE: u8 = 0x0A;
        pub const MATCH_END: u8 = 0x0B;
        pub const PLAYER_BLOCK: u8 = 0x0F;
        pub const RESPAWN_REQUEST: u8 = 0x1C;
        pub const RESPAWN: u8 = 0x1D;
        pub const SEEDED_RESPAWN_REQUEST: u8 = 0x1E;
        pub const SEEDED_RESPAWN: u8 = 0x1F;
        pub const EQUIPMENT: u8 = 0x21;
        pub const PICKUP_STATES: u8 = 0x22;
        pub const WEAPON_DROP: u8 = 0x23;
        pub const REMOVE_PLAYER_OBJECT: u8 = 0x24;
        pub const SNAPSHOT: u8 = 0x25;
        pub const SNAPSHOT_REQUEST: u8 = 0x26;
        pub const EFFECT_MASK: u8 = 0x27;
        pub const FOCUS_OBJECT: u8 = 0x28;
        pub const PLAYER_VALUES: u8 = 0x29;
        pub const FRAGMENT: u8 = 0x2E;
        pub const TILE_ACK: u8 = 0x31;
        pub const SNAPSHOT_ACK: u8 = 0x32;
        pub const ENTRY_WORDS: u8 = 0x37;
        pub const PEER_BLOCKS_REQUEST: u8 = 0x38;
        pub const ROSTER_SUMMARY: u8 = 0x3A;
        pub const RESTORE_STATE: u8 = 0x3C;
        pub const SIGNAL_3E: u8 = 0x3E;
        pub const STAMP: u8 = 0x3F;
        pub const TEAM_BITS: u8 = 0x40;
        pub const OBJECT_STATES: u8 = 0x44;
        pub const STATE_LIST: u8 = 0x46;
    }

    /// `code, key, sequence` prefix (sender 0x353820); the payload follows at offset 4.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Header {
        pub code: u8,
        pub key: u8,
        pub sequence: u16,
    }

    /// Per-slot reliable sequence: increments and skips 0 (0x353930..0x353951).
    pub fn next_reliable_sequence(current: u16) -> u16 {
        match current.wrapping_add(1) {
            0 => 1,
            n => n,
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PlayerRecord {
        pub player: u8,
        pub flags: u8,
        pub bits_a: u32,
        pub bits_b: u32,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Snapshot {
        pub game_state: u8,
        pub time_a: i32,
        pub time_b: i32,
        /// Present only when `game_state == 8`.
        pub extra: Option<u8>,
        pub players: Vec<PlayerRecord>,
        pub team_values: Vec<u32>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct AmmoPair {
        pub index: u8,
        pub reserve: u16,
        pub clip: u16,
    }

    /// 24-byte record of code 0x21.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Equipment {
        pub player: u8,
        pub ext_94: u8,
        pub primary: [AmmoPair; 2],
        pub field_594: u8,
        pub secondary: [AmmoPair; 2],
        pub field_1bc: u8,
    }

    /// 36-byte record of code 0x44.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ObjectState {
        pub sequence: u8,
        pub active: u8,
        pub position: [F24; 3],
        pub transform: [u32; 4],
        pub velocity: [F24; 3],
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum TableEntry {
        None,
        Value(u16),
        Owner(u8),
    }

    /// Typed gameplay packet. Codes without a typed layout are carried as `Raw`.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Packet {
        Leave,
        SessionCounter(u8),
        SessionEvent {
            a: u8,
            b: u8,
        },
        SlotType(u8),
        MatchEnd,
        PlayerBlock {
            player: u8,
            words: Box<[u32; 31]>,
        },
        /// 0x1C/0x1D short form (flags bit0 set): player and side only.
        RespawnShort {
            code: u8,
            flags: u8,
            player: u8,
            side: u8,
        },
        /// 0x1C/0x1D extended form (flags bit0 clear).
        RespawnSeeded {
            code: u8,
            flags: u8,
            player: u8,
            side: u8,
            object_id: u16,
            ext_678: u16,
            seed: u32,
        },
        /// 0x1E/0x1F.
        SeededRespawn {
            code: u8,
            player: u8,
            value: u16,
            seed: u32,
        },
        Equipment(Vec<Equipment>),
        /// 0x22: (entry, state, owner 0xFF = none).
        PickupStates(Vec<(u8, u8, u8)>),
        WeaponDrop {
            player: u8,
            value: u16,
        },
        RemovePlayerObject {
            player: u8,
            value: u16,
        },
        Snapshot(Snapshot),
        SnapshotRequest,
        EffectMask(u32),
        FocusObject(ObjectRef),
        PlayerValues {
            player: u8,
            values: Box<[u16; 22]>,
        },
        TileAck,
        SnapshotAck,
        EntryWords(Vec<u16>),
        PeerBlocksRequest,
        /// 0x3A: header bytes, per-player triples, lobby availability.
        RosterSummary {
            slot_2fa: u8,
            session_id: u8,
            players: Vec<[u8; 3]>,
            lobby_available: u8,
        },
        RestoreState {
            a: u8,
            b: u8,
        },
        Signal3e,
        Stamp {
            kind: u8,
            value: u8,
            slot: u8,
            stamp: u32,
        },
        TeamBits {
            player: u8,
            value: u32,
            team_bits: u32,
        },
        ObjectStates(Vec<ObjectState>),
        StateList(Vec<u8>),
        Fragment(Fragment),
        Raw {
            code: u8,
            payload: Vec<u8>,
        },
    }

    /// 0x2E fragment envelope (after the 4-byte prefix).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Fragment {
        pub message_seq: u32,
        pub inner_code: u32,
        pub index: u8,
        pub total: u8,
        pub chunk: Vec<u8>,
    }

    impl Packet {
        pub fn code(&self) -> u8 {
            use code::*;
            match self {
                Packet::Leave => LEAVE,
                Packet::SessionCounter(_) => SESSION_COUNTER,
                Packet::SessionEvent { .. } => SESSION_EVENT,
                Packet::SlotType(_) => SLOT_TYPE,
                Packet::MatchEnd => MATCH_END,
                Packet::PlayerBlock { .. } => PLAYER_BLOCK,
                Packet::RespawnShort { code, .. } | Packet::RespawnSeeded { code, .. } => *code,
                Packet::SeededRespawn { code, .. } => *code,
                Packet::Equipment(_) => EQUIPMENT,
                Packet::PickupStates(_) => PICKUP_STATES,
                Packet::WeaponDrop { .. } => WEAPON_DROP,
                Packet::RemovePlayerObject { .. } => REMOVE_PLAYER_OBJECT,
                Packet::Snapshot(_) => SNAPSHOT,
                Packet::SnapshotRequest => SNAPSHOT_REQUEST,
                Packet::EffectMask(_) => EFFECT_MASK,
                Packet::FocusObject(_) => FOCUS_OBJECT,
                Packet::PlayerValues { .. } => PLAYER_VALUES,
                Packet::TileAck => TILE_ACK,
                Packet::SnapshotAck => SNAPSHOT_ACK,
                Packet::EntryWords(_) => ENTRY_WORDS,
                Packet::PeerBlocksRequest => PEER_BLOCKS_REQUEST,
                Packet::RosterSummary { .. } => ROSTER_SUMMARY,
                Packet::RestoreState { .. } => RESTORE_STATE,
                Packet::Signal3e => SIGNAL_3E,
                Packet::Stamp { .. } => STAMP,
                Packet::TeamBits { .. } => TEAM_BITS,
                Packet::ObjectStates(_) => OBJECT_STATES,
                Packet::StateList(_) => STATE_LIST,
                Packet::Fragment(_) => FRAGMENT,
                Packet::Raw { code, .. } => *code,
            }
        }

        /// Payload bytes (after the 4-byte prefix).
        pub fn encode_payload(&self) -> Vec<u8> {
            let mut w = Writer::new();
            match self {
                Packet::Leave
                | Packet::MatchEnd
                | Packet::SnapshotRequest
                | Packet::TileAck
                | Packet::SnapshotAck
                | Packet::PeerBlocksRequest
                | Packet::Signal3e => {}
                Packet::SessionCounter(v) | Packet::SlotType(v) => {
                    w.u8(*v);
                }
                Packet::SessionEvent { a, b } | Packet::RestoreState { a, b } => {
                    w.u8(*a).u8(*b);
                }
                Packet::PlayerBlock { player, words } => {
                    w.u8(*player);
                    for word in words.iter() {
                        w.u32(*word);
                    }
                }
                Packet::RespawnShort {
                    flags,
                    player,
                    side,
                    ..
                } => {
                    w.u8(*flags | 1).u8(*player).u8(*side);
                }
                Packet::RespawnSeeded {
                    flags,
                    player,
                    side,
                    object_id,
                    ext_678,
                    seed,
                    ..
                } => {
                    w.u8(*flags & !1)
                        .u8(*player)
                        .u8(*side)
                        .u16(*object_id)
                        .u16(*ext_678)
                        .u32(*seed);
                }
                Packet::SeededRespawn {
                    player,
                    value,
                    seed,
                    ..
                } => {
                    w.u8(*player).u16(*value).u32(*seed);
                }
                Packet::Equipment(records) => {
                    w.u8(records.len() as u8);
                    for e in records {
                        w.u8(e.player).u8(e.ext_94);
                        for p in &e.primary {
                            w.u8(p.index).u16(p.reserve).u16(p.clip);
                        }
                        w.u8(e.field_594);
                        for p in &e.secondary {
                            w.u8(p.index).u16(p.reserve).u16(p.clip);
                        }
                        w.u8(e.field_1bc);
                    }
                }
                Packet::PickupStates(entries) => {
                    w.u8(entries.len() as u8);
                    for (a, b, c) in entries {
                        w.u8(*a).u8(*b).u8(*c);
                    }
                }
                Packet::WeaponDrop { player, value }
                | Packet::RemovePlayerObject { player, value } => {
                    w.u8(*player).u16(*value);
                }
                Packet::Snapshot(s) => {
                    w.u8(s.players.len() as u8)
                        .u8(s.game_state)
                        .i32(s.time_a)
                        .i32(s.time_b);
                    if s.game_state == 8 {
                        w.u8(s.extra.unwrap_or(0));
                    }
                    for p in &s.players {
                        w.u8(p.player).u8(p.flags).u32(p.bits_a).u32(p.bits_b);
                    }
                    w.u8(s.team_values.len() as u8);
                    for v in &s.team_values {
                        w.u32(*v);
                    }
                }
                Packet::EffectMask(v) => {
                    w.u32(*v);
                }
                Packet::FocusObject(r) => r.encode(&mut w),
                Packet::PlayerValues { player, values } => {
                    w.u8(*player);
                    for v in values.iter() {
                        w.u16(*v);
                    }
                }
                Packet::EntryWords(words) => {
                    w.u8(words.len() as u8);
                    for v in words {
                        w.u16(*v);
                    }
                }
                Packet::RosterSummary {
                    slot_2fa,
                    session_id,
                    players,
                    lobby_available,
                } => {
                    w.u8(players.len() as u8).u8(*slot_2fa).u8(*session_id);
                    for p in players {
                        w.bytes(p);
                    }
                    w.u8(*lobby_available);
                }
                Packet::Stamp {
                    kind,
                    value,
                    slot,
                    stamp,
                } => {
                    w.u8(*kind).u8(*value).u8(*slot).u32(*stamp);
                }
                Packet::TeamBits {
                    player,
                    value,
                    team_bits,
                } => {
                    w.u8(*player).u32(*value).u32(*team_bits);
                }
                Packet::ObjectStates(records) => {
                    w.u8(records.len() as u8);
                    for o in records {
                        w.u8(o.sequence).u8(o.active).v24(o.position);
                        for t in o.transform {
                            w.u32(t);
                        }
                        w.v24(o.velocity);
                    }
                }
                Packet::StateList(states) => {
                    w.u8(states.len() as u8).bytes(states);
                }
                Packet::Fragment(f) => {
                    w.u32(f.message_seq)
                        .u32(f.inner_code)
                        .u16(f.chunk.len() as u16)
                        .u8(f.index)
                        .u8(f.total);
                    w.bytes(&f.chunk);
                }
                Packet::Raw { payload, .. } => {
                    w.bytes(payload);
                }
            }
            w.buf
        }

        /// Full packet: 4-byte prefix plus payload. `key` and `sequence` are transport fields.
        pub fn encode(&self, key: u8, sequence: u16) -> Vec<u8> {
            let mut w = Writer::new();
            w.u8(self.code())
                .u8(key)
                .u16(sequence)
                .bytes(&self.encode_payload());
            w.buf
        }

        pub fn decode(data: &[u8]) -> Result<(Header, Packet)> {
            let mut r = Reader::new(data);
            let header = Header {
                code: r.u8()?,
                key: r.u8()?,
                sequence: r.u16()?,
            };
            Ok((header, Self::decode_payload(header.code, r.rest())?))
        }

        pub fn decode_payload(code: u8, payload: &[u8]) -> Result<Packet> {
            use self::code as c;
            let mut r = Reader::new(payload);
            let packet = match code {
                c::LEAVE => Packet::Leave,
                c::MATCH_END => Packet::MatchEnd,
                c::SNAPSHOT_REQUEST => Packet::SnapshotRequest,
                c::TILE_ACK => Packet::TileAck,
                c::SNAPSHOT_ACK => Packet::SnapshotAck,
                c::PEER_BLOCKS_REQUEST => Packet::PeerBlocksRequest,
                c::SIGNAL_3E => Packet::Signal3e,
                c::SESSION_COUNTER => Packet::SessionCounter(r.u8()?),
                c::SLOT_TYPE => Packet::SlotType(r.u8()?),
                c::SESSION_EVENT => Packet::SessionEvent {
                    a: r.u8()?,
                    b: r.u8()?,
                },
                c::RESTORE_STATE => Packet::RestoreState {
                    a: r.u8()?,
                    b: r.u8()?,
                },
                c::PLAYER_BLOCK => {
                    let player = r.u8()?;
                    let mut words = Box::new([0u32; 31]);
                    for word in words.iter_mut() {
                        *word = r.u32()?;
                    }
                    Packet::PlayerBlock { player, words }
                }
                c::RESPAWN_REQUEST | c::RESPAWN => {
                    let flags = r.u8()?;
                    let player = r.u8()?;
                    let side = r.u8()?;
                    if flags & 1 != 0 {
                        Packet::RespawnShort {
                            code,
                            flags,
                            player,
                            side,
                        }
                    } else {
                        Packet::RespawnSeeded {
                            code,
                            flags,
                            player,
                            side,
                            object_id: r.u16()?,
                            ext_678: r.u16()?,
                            seed: r.u32()?,
                        }
                    }
                }
                c::SEEDED_RESPAWN_REQUEST | c::SEEDED_RESPAWN => Packet::SeededRespawn {
                    code,
                    player: r.u8()?,
                    value: r.u16()?,
                    seed: r.u32()?,
                },
                c::EQUIPMENT => {
                    let count = r.u8()?;
                    let mut out = Vec::with_capacity(count as usize);
                    let pair = |r: &mut Reader<'_>| -> Result<AmmoPair> {
                        Ok(AmmoPair {
                            index: r.u8()?,
                            reserve: r.u16()?,
                            clip: r.u16()?,
                        })
                    };
                    for _ in 0..count {
                        let player = r.u8()?;
                        let ext_94 = r.u8()?;
                        let primary = [pair(&mut r)?, pair(&mut r)?];
                        let field_594 = r.u8()?;
                        let secondary = [pair(&mut r)?, pair(&mut r)?];
                        let field_1bc = r.u8()?;
                        out.push(Equipment {
                            player,
                            ext_94,
                            primary,
                            field_594,
                            secondary,
                            field_1bc,
                        });
                    }
                    Packet::Equipment(out)
                }
                c::PICKUP_STATES => {
                    let count = r.u8()?;
                    let mut out = Vec::new();
                    for _ in 0..count {
                        out.push((r.u8()?, r.u8()?, r.u8()?));
                    }
                    Packet::PickupStates(out)
                }
                c::WEAPON_DROP => Packet::WeaponDrop {
                    player: r.u8()?,
                    value: r.u16()?,
                },
                c::REMOVE_PLAYER_OBJECT => Packet::RemovePlayerObject {
                    player: r.u8()?,
                    value: r.u16()?,
                },
                c::SNAPSHOT => {
                    let players = r.u8()?;
                    let game_state = r.u8()?;
                    let time_a = r.i32()?;
                    let time_b = r.i32()?;
                    let extra = if game_state == 8 { Some(r.u8()?) } else { None };
                    let mut records = Vec::new();
                    for _ in 0..players {
                        records.push(PlayerRecord {
                            player: r.u8()?,
                            flags: r.u8()?,
                            bits_a: r.u32()?,
                            bits_b: r.u32()?,
                        });
                    }
                    let teams = r.u8()?;
                    let mut team_values = Vec::new();
                    for _ in 0..teams {
                        team_values.push(r.u32()?);
                    }
                    Packet::Snapshot(Snapshot {
                        game_state,
                        time_a,
                        time_b,
                        extra,
                        players: records,
                        team_values,
                    })
                }
                c::EFFECT_MASK => Packet::EffectMask(r.u32()?),
                c::FOCUS_OBJECT => Packet::FocusObject(ObjectRef::decode(&mut r)?),
                c::PLAYER_VALUES => {
                    let player = r.u8()?;
                    let mut values = Box::new([0u16; 22]);
                    for v in values.iter_mut() {
                        *v = r.u16()?;
                    }
                    Packet::PlayerValues { player, values }
                }
                c::ENTRY_WORDS => {
                    let count = r.u8()?;
                    let mut out = Vec::new();
                    for _ in 0..count {
                        out.push(r.u16()?);
                    }
                    Packet::EntryWords(out)
                }
                c::ROSTER_SUMMARY => {
                    let count = r.u8()?;
                    let slot_2fa = r.u8()?;
                    let session_id = r.u8()?;
                    let mut players = Vec::new();
                    for _ in 0..count {
                        players.push([r.u8()?, r.u8()?, r.u8()?]);
                    }
                    Packet::RosterSummary {
                        slot_2fa,
                        session_id,
                        players,
                        lobby_available: r.u8()?,
                    }
                }
                c::STAMP => Packet::Stamp {
                    kind: r.u8()?,
                    value: r.u8()?,
                    slot: r.u8()?,
                    stamp: r.u32()?,
                },
                c::TEAM_BITS => Packet::TeamBits {
                    player: r.u8()?,
                    value: r.u32()?,
                    team_bits: r.u32()?,
                },
                c::OBJECT_STATES => {
                    let count = r.u8()?;
                    let mut out = Vec::new();
                    for _ in 0..count {
                        let sequence = r.u8()?;
                        let active = r.u8()?;
                        let position = r.v24()?;
                        let transform = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                        let velocity = r.v24()?;
                        out.push(ObjectState {
                            sequence,
                            active,
                            position,
                            transform,
                            velocity,
                        });
                    }
                    Packet::ObjectStates(out)
                }
                c::STATE_LIST => {
                    let count = r.u8()? as usize;
                    Packet::StateList(r.bytes(count)?.to_vec())
                }
                c::FRAGMENT => {
                    let message_seq = r.u32()?;
                    let inner_code = r.u32()?;
                    let len = r.u16()? as usize;
                    let index = r.u8()?;
                    let total = r.u8()?;
                    if len > FRAGMENT_CHUNK {
                        return Err(CodecError::BadFragment("chunk longer than 0xEF"));
                    }
                    Packet::Fragment(Fragment {
                        message_seq,
                        inner_code,
                        index,
                        total,
                        chunk: r.bytes(len)?.to_vec(),
                    })
                }
                _ => {
                    return Ok(Packet::Raw {
                        code,
                        payload: payload.to_vec(),
                    });
                }
            };
            Ok(packet)
        }
    }

    /// Split `data` into fragments of at most 239 bytes (sender 0x353FE0). Empty input sends nothing
    /// but still consumes a message sequence number in the original (the caller increments it).
    pub fn fragment(message_seq: u32, inner_code: u32, data: &[u8]) -> Vec<Fragment> {
        let total = data.len().div_ceil(FRAGMENT_CHUNK);
        data.chunks(FRAGMENT_CHUNK)
            .enumerate()
            .map(|(i, chunk)| Fragment {
                message_seq,
                inner_code,
                index: i as u8,
                total: total as u8,
                chunk: chunk.to_vec(),
            })
            .collect()
    }

    /// Reassembles fragments of one message sequence. Out-of-order arrival is accepted; a new
    /// message sequence discards a partial message.
    ///
    /// ```
    /// use tsfp_network::syslink_codec::game::{Reassembler, fragment};
    /// let mut receiver = Reassembler::default();
    /// let chunks = fragment(7, 0x3b, &[42; 240]);
    /// assert!(receiver.push(&chunks[1]).unwrap().is_none());
    /// assert_eq!(receiver.push(&chunks[0]).unwrap(), Some((0x3b, vec![42; 240])));
    /// ```
    #[derive(Default)]
    pub struct Reassembler {
        current: Option<(u32, u32, Vec<Option<Vec<u8>>>)>,
    }

    impl Reassembler {
        /// Returns `(inner_code, data)` when the last missing chunk arrives.
        pub fn push(&mut self, f: &Fragment) -> Result<Option<(u32, Vec<u8>)>> {
            if f.total == 0 || f.index >= f.total {
                return Err(CodecError::BadFragment("index outside total"));
            }
            let reset = match &self.current {
                Some((seq, _, slots)) => *seq != f.message_seq || slots.len() != f.total as usize,
                None => true,
            };
            if reset {
                self.current = Some((f.message_seq, f.inner_code, vec![None; f.total as usize]));
            }
            let (_, code, slots) = self.current.as_mut().expect("set above");
            slots[f.index as usize] = Some(f.chunk.clone());
            if slots.iter().all(Option::is_some) {
                let data: Vec<u8> = slots.iter().flatten().flatten().copied().collect();
                let code = *code;
                self.current = None;
                return Ok(Some((code, data)));
            }
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Layer 4: FESL LAN advertisement (port 9999)
// ---------------------------------------------------------------------------------------------
pub mod fesl {
    use super::*;

    pub const PORT: u16 = 9999;
    pub const PACKET_LEN: usize = 384;
    pub const MAGIC: [u8; 3] = *b"gEA";

    /// 384-byte `gEA` announcement (builder 0x3C7B90) or query (`?` at byte 8, builder 0x3C7700).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Lan {
        Query,
        Announce {
            interval_seconds: u8,
            word: u32,
            name: Vec<u8>,
            product: Vec<u8>,
            attributes: Vec<u8>,
            endpoint: Vec<u8>,
        },
    }

    fn put(buf: &mut [u8], offset: usize, max: usize, text: &[u8]) {
        let n = text.len().min(max - 1);
        buf[offset..offset + n].copy_from_slice(&text[..n]);
    }

    fn take(buf: &[u8], offset: usize, max: usize) -> Vec<u8> {
        let field = &buf[offset..offset + max];
        let end = field.iter().position(|&b| b == 0).unwrap_or(max);
        field[..end].to_vec()
    }

    pub fn encode(packet: &Lan) -> Vec<u8> {
        let mut buf = vec![0u8; PACKET_LEN];
        buf[..3].copy_from_slice(&MAGIC);
        match packet {
            Lan::Query => buf[8] = b'?',
            Lan::Announce {
                interval_seconds,
                word,
                name,
                product,
                attributes,
                endpoint,
            } => {
                buf[3] = (*interval_seconds).clamp(2, 250);
                buf[4..8].copy_from_slice(&word.to_be_bytes());
                put(&mut buf, 8, 32, name);
                put(&mut buf, 40, 32, product);
                put(&mut buf, 72, 192, attributes);
                put(&mut buf, 264, 120, endpoint);
            }
        }
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Lan> {
        if data.len() < PACKET_LEN {
            return Err(CodecError::BadLength("gEA packet < 384"));
        }
        if data[..3] != MAGIC {
            return Err(CodecError::BadMagic);
        }
        if data[8] == b'?' && data[9] == 0 {
            return Ok(Lan::Query);
        }
        Ok(Lan::Announce {
            interval_seconds: data[3],
            word: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            name: take(data, 8, 32),
            product: take(data, 40, 32),
            attributes: take(data, 72, 192),
            endpoint: take(data, 264, 120),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::game::*;
    use super::*;

    fn roundtrip(packet: Packet) {
        let bytes = packet.encode(0x35, 7);
        let (header, decoded) = Packet::decode(&bytes).expect("decode");
        assert_eq!(
            header,
            Header {
                code: packet.code(),
                key: 0x35,
                sequence: 7
            }
        );
        assert_eq!(decoded, packet);
    }

    #[test]
    fn f24_keeps_top_three_bytes() {
        let value = 1234.5678_f32;
        let back = f24_to_f32(f32_to_f24(value));
        assert_eq!(back.to_bits(), value.to_bits() & !0xFF);
        let mut w = Writer::new();
        w.f24(f32_to_f24(1.0));
        assert_eq!(w.buf, vec![0x00, 0x80, 0x3F]);
        assert_eq!(f24_to_f32(Reader::new(&w.buf).f24().unwrap()), 1.0);
    }

    #[test]
    fn refs_roundtrip_and_unknown_tag_is_null() {
        for r in [
            ObjectRef::Null,
            ObjectRef::Player(3),
            ObjectRef::Id(0xDEADBEEF),
            ObjectRef::Linked(0xFF),
        ] {
            let mut w = Writer::new();
            r.encode(&mut w);
            assert_eq!(ObjectRef::decode(&mut Reader::new(&w.buf)).unwrap(), r);
        }
        assert_eq!(
            ObjectRef::decode(&mut Reader::new(&[9])).unwrap(),
            ObjectRef::Null
        );
        assert_eq!(ObjectRef::Player(1).encode_len(), 2);
    }

    impl ObjectRef {
        fn encode_len(self) -> usize {
            let mut w = Writer::new();
            self.encode(&mut w);
            w.buf.len()
        }
    }

    #[test]
    fn gameplay_packets_roundtrip() {
        roundtrip(Packet::Leave);
        roundtrip(Packet::SessionCounter(4));
        roundtrip(Packet::SessionEvent { a: 1, b: 2 });
        roundtrip(Packet::WeaponDrop {
            player: 2,
            value: 0x1234,
        });
        roundtrip(Packet::EffectMask(0xA5A5_0F0F));
        roundtrip(Packet::FocusObject(ObjectRef::Id(77)));
        roundtrip(Packet::EntryWords(vec![1, 2, 3, 0xFFFF]));
        roundtrip(Packet::StateList(vec![0, 1, 2, 3]));
        roundtrip(Packet::Stamp {
            kind: 1,
            value: 2,
            slot: 3,
            stamp: 0xCAFEBABE,
        });
        roundtrip(Packet::TeamBits {
            player: 1,
            value: 5,
            team_bits: 0x1F0,
        });
        roundtrip(Packet::RespawnShort {
            code: code::RESPAWN,
            flags: 1,
            player: 2,
            side: 1,
        });
        roundtrip(Packet::RespawnSeeded {
            code: code::RESPAWN_REQUEST,
            flags: 0,
            player: 2,
            side: 1,
            object_id: 9,
            ext_678: 10,
            seed: 11,
        });
        roundtrip(Packet::RosterSummary {
            slot_2fa: 1,
            session_id: 2,
            players: vec![[1, 2, 3], [4, 5, 6]],
            lobby_available: 1,
        });
        roundtrip(Packet::Raw {
            code: 0x06,
            payload: vec![1, 2, 3],
        });
    }

    #[test]
    fn measured_total_lengths() {
        // 0x0F: total 0x81 (u8 + 31 dwords + 4 byte prefix).
        let p = Packet::PlayerBlock {
            player: 1,
            words: Box::new([7; 31]),
        };
        assert_eq!(p.encode(0, 0).len(), 0x81);
        roundtrip(p);
        // 0x29: 45 payload bytes.
        let p = Packet::PlayerValues {
            player: 0,
            values: Box::new([9; 22]),
        };
        assert_eq!(p.encode_payload().len(), 45);
        roundtrip(p);
        // 0x3F total 11, 0x40 total 13, 0x1E total 11.
        assert_eq!(
            Packet::Stamp {
                kind: 0,
                value: 0,
                slot: 0,
                stamp: 0
            }
            .encode(0, 0)
            .len(),
            11
        );
        assert_eq!(
            Packet::TeamBits {
                player: 0,
                value: 0,
                team_bits: 0
            }
            .encode(0, 0)
            .len(),
            13
        );
        assert_eq!(
            Packet::SeededRespawn {
                code: 0x1E,
                player: 0,
                value: 0,
                seed: 0
            }
            .encode(0, 0)
            .len(),
            11
        );
        // 0x23 total 7, short respawn total 7, extended total 15.
        assert_eq!(
            Packet::WeaponDrop {
                player: 0,
                value: 0
            }
            .encode(0, 0)
            .len(),
            7
        );
        assert_eq!(
            Packet::RespawnShort {
                code: 0x1C,
                flags: 1,
                player: 0,
                side: 0
            }
            .encode(0, 0)
            .len(),
            7
        );
        let ext = Packet::RespawnSeeded {
            code: 0x1C,
            flags: 0,
            player: 0,
            side: 0,
            object_id: 0,
            ext_678: 0,
            seed: 0,
        };
        assert_eq!(ext.encode(0, 0).len(), 15);
    }

    #[test]
    fn snapshot_layout_and_total_length() {
        let plain = Snapshot {
            game_state: 3,
            time_a: 600,
            time_b: -1,
            extra: None,
            players: vec![
                PlayerRecord {
                    player: 0,
                    flags: 1,
                    bits_a: 0x3F80_0000,
                    bits_b: 2,
                },
                PlayerRecord {
                    player: 1,
                    flags: 2,
                    bits_a: 3,
                    bits_b: 4,
                },
            ],
            team_values: vec![10, 20],
        };
        // payload = 10 + (state==8) + 10*players + 1 + 4*teams
        assert_eq!(
            Packet::Snapshot(plain.clone()).encode_payload().len(),
            10 + 20 + 1 + 8
        );
        roundtrip(Packet::Snapshot(plain.clone()));
        let ended = Snapshot {
            game_state: 8,
            extra: Some(9),
            ..plain
        };
        assert_eq!(
            Packet::Snapshot(ended.clone()).encode_payload().len(),
            10 + 1 + 20 + 1 + 8
        );
        roundtrip(Packet::Snapshot(ended));
    }

    #[test]
    fn equipment_and_object_state_records() {
        let pair = |i| AmmoPair {
            index: i,
            reserve: 100 + i as u16,
            clip: 10,
        };
        let e = Equipment {
            player: 1,
            ext_94: 2,
            primary: [pair(1), pair(2)],
            field_594: 3,
            secondary: [pair(3), pair(4)],
            field_1bc: 4,
        };
        let p = Packet::Equipment(vec![e.clone(), e]);
        assert_eq!(p.encode_payload().len(), 1 + 2 * 24);
        roundtrip(p);
        let o = ObjectState {
            sequence: 1,
            active: 1,
            position: [f32_to_f24(1.5), f32_to_f24(-2.0), f32_to_f24(0.25)],
            transform: [1, 2, 3, 4],
            velocity: [F24(0), F24(1), F24(2)],
        };
        let p = Packet::ObjectStates(vec![o]);
        assert_eq!(p.encode_payload().len(), 1 + 36);
        roundtrip(p);
    }

    #[test]
    fn truncated_packets_are_errors_not_panics() {
        let full = Packet::Stamp {
            kind: 1,
            value: 2,
            slot: 3,
            stamp: 4,
        }
        .encode(0, 0);
        for cut in 4..full.len() {
            assert!(matches!(
                Packet::decode(&full[..cut]),
                Err(CodecError::Truncated { .. })
            ));
        }
    }

    #[test]
    fn fragments_split_at_239_and_reassemble_out_of_order() {
        let data: Vec<u8> = (0..600u32).map(|i| i as u8).collect();
        let fragments = fragment(5, 0x3B, &data);
        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments[0].chunk.len(), 0xEF);
        assert_eq!(fragments[2].chunk.len(), 600 - 2 * 0xEF);
        let mut re = Reassembler::default();
        let mut out = None;
        for i in [2usize, 0, 1] {
            let wire = Packet::Fragment(fragments[i].clone()).encode(0, 1);
            let (_, decoded) = Packet::decode(&wire).unwrap();
            let Packet::Fragment(f) = decoded else {
                panic!("not a fragment")
            };
            assert_eq!(f.chunk.len() + 16, wire.len());
            out = re.push(&f).unwrap();
        }
        assert_eq!(out, Some((0x3B, data)));
        assert!(fragment(1, 1, &[]).is_empty());
    }

    #[test]
    fn fragment_reassembly_size_and_interruption_grid() {
        for len in 1_usize..=1024 {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let chunks = fragment(len as u32, 0x3b, &data);
            assert_eq!(chunks.len(), len.div_ceil(FRAGMENT_CHUNK));
            assert!(chunks
                .iter()
                .all(|f| !f.chunk.is_empty() && f.chunk.len() <= 239));
            let mut receiver = Reassembler::default();
            for (position, chunk) in chunks.iter().rev().enumerate() {
                let result = receiver.push(chunk).unwrap();
                if position + 1 == chunks.len() {
                    assert_eq!(result, Some((0x3b, data.clone())), "length {len}");
                } else {
                    assert!(result.is_none(), "length {len}");
                }
            }
            if chunks.len() > 1 {
                let mut receiver = Reassembler::default();
                assert!(receiver.push(&chunks[0]).unwrap().is_none());
                let next = fragment((len + 1) as u32, 0x45, &data);
                for chunk in &next[..next.len() - 1] {
                    assert!(receiver.push(chunk).unwrap().is_none());
                }
                assert_eq!(
                    receiver.push(next.last().unwrap()).unwrap(),
                    Some((0x45, data))
                );
            }
        }
    }

    #[test]
    fn reliable_sequence_skips_zero() {
        assert_eq!(next_reliable_sequence(0), 1);
        assert_eq!(next_reliable_sequence(0xFFFF), 1);
        assert_eq!(next_reliable_sequence(41), 42);
    }

    #[test]
    fn control_messages_roundtrip_and_have_measured_sizes() {
        let join = vdp::Control::JoinRequest {
            players: 2,
            user_tag: 7,
            user_name: b"Player One".to_vec(),
        };
        assert_eq!(join.encode_header().len(), 0x34);
        for c in [
            join,
            vdp::Control::JoinAccept {
                assigned_slot: 3,
                host_slot: 0,
                host_players: 1,
            },
            vdp::Control::JoinDeny,
            vdp::Control::PeerRegister {
                slot: 3,
                user_tag: -1,
                user_name: Vec::new(),
            },
            vdp::Control::PeerAck,
            vdp::Control::PingRequest,
            vdp::Control::PingReply,
            vdp::Control::GameData(Packet::Leave.encode(0, 0)),
        ] {
            let datagram = c.to_datagram().unwrap();
            let frame = vdp::Frame::decode(&datagram).unwrap();
            assert_eq!(
                vdp::ControlKind::try_from(u32::from_le_bytes(
                    frame.header[..4].try_into().unwrap()
                ))
                .unwrap(),
                c.kind()
            );
            assert_eq!(vdp::Control::decode_header(&frame.header).unwrap(), c);
        }
        // Fixed control datagram = u16 + 0x34 header.
        assert_eq!(
            vdp::Control::JoinDeny.to_datagram().unwrap().len(),
            2 + 0x34
        );
    }

    #[test]
    fn fixed_controls_reject_truncation_and_keep_unknown_padding_opaque() {
        use vdp::Control;

        for control in [
            Control::JoinRequest {
                players: 1,
                user_tag: -1,
                user_name: b"p".to_vec(),
            },
            Control::JoinAccept {
                assigned_slot: 1,
                host_slot: 0,
                host_players: 1,
            },
            Control::JoinDeny,
            Control::PeerRegister {
                slot: 1,
                user_tag: -1,
                user_name: b"p".to_vec(),
            },
            Control::PeerAck,
        ] {
            let canonical = control.encode_header();
            assert_eq!(canonical.len(), vdp::CONTROL_LEN);
            for cut in [4, 15, vdp::CONTROL_LEN - 1] {
                assert!(
                    vdp::Control::decode_header(&canonical[..cut]).is_err(),
                    "{control:?} at {cut}"
                );
            }
        }
        // Padding bytes were partly uninitialised in the original; the typed parser ignores them.
        for (control, padding_start) in [
            (
                Control::JoinAccept {
                    assigned_slot: 1,
                    host_slot: 0,
                    host_players: 1,
                },
                16,
            ),
            (Control::JoinDeny, 4),
            (Control::PeerAck, 4),
        ] {
            let mut wire = control.encode_header();
            wire[padding_start..].fill(0xA5);
            assert_eq!(vdp::Control::decode_header(&wire), Ok(control));
        }
        assert_eq!(
            vdp::ControlKind::try_from(0xDEAD_B33F),
            Ok(vdp::ControlKind::GameData)
        );
        assert!(vdp::ControlKind::try_from(0x1234_5678).is_err());
    }

    #[test]
    fn user_name_is_truncated_to_31_bytes_unlike_the_original_unbounded_copy() {
        let long = vec![b'x'; 80];
        let c = vdp::Control::JoinRequest {
            players: 1,
            user_tag: 1,
            user_name: long,
        };
        assert_eq!(c.encode_header().len(), 0x34);
        let decoded = vdp::Control::decode_header(&c.encode_header()).unwrap();
        let vdp::Control::JoinRequest { user_name, .. } = decoded else {
            panic!()
        };
        assert_eq!(user_name.len(), 31);
    }

    #[test]
    fn game_data_wraps_a_gameplay_packet_and_header_length_is_packet_plus_four() {
        let packet = Packet::SessionCounter(9).encode(1, 2);
        let datagram = vdp::Control::GameData(packet.clone())
            .to_datagram()
            .unwrap();
        let frame = vdp::Frame::decode(&datagram).unwrap();
        assert_eq!(frame.header.len(), packet.len() + 4);
        assert_eq!(
            u16::from_le_bytes([datagram[0], datagram[1]]) as usize,
            packet.len() + 4
        );
        assert_eq!(&datagram[2..6], &0xDEAD_B33Fu32.to_le_bytes());
    }

    #[test]
    fn frame_with_voice_tail_and_size_limits() {
        let frame = vdp::Frame {
            header: vec![1, 2, 3, 4],
            tail: vec![9; 11],
        };
        let wire = frame.encode().unwrap();
        assert_eq!(wire.len(), 2 + 4 + 11);
        assert_eq!(vdp::Frame::decode(&wire).unwrap(), frame);
        assert!(vdp::Frame::decode(&[0, 0, 0]).is_err());
        assert!(vdp::Frame::decode(&vec![0; 0x201]).is_err());
        assert!(matches!(
            vdp::Frame::decode(&[9, 0, 1, 2, 3, 4]),
            Err(CodecError::Truncated { .. })
        ));
        let oversized = vdp::Frame {
            header: vec![0; u16::MAX as usize + 1],
            tail: Vec::new(),
        };
        assert!(matches!(oversized.encode(), Err(CodecError::BadLength(_))));
    }

    #[test]
    fn discovery_datagrams_roundtrip() {
        use discovery::*;
        assert_eq!(encode(&Datagram::Query), 0xFFFF_FFFBu32.to_le_bytes());
        let advert = Advert {
            game_a: 1,
            game_b: 2,
            game_c: -3,
            free_slots: 15,
            name: b"Test Lobby".to_vec(),
            random_id: [1; 8],
            xnaddr: [2; 36],
            session_id: [3; 8],
            key: [4; 16],
        };
        let wire = encode(&Datagram::Advert(advert.clone()));
        assert_eq!(wire.len(), ADVERT_LEN);
        assert_eq!(decode(&wire).unwrap(), Datagram::Advert(advert));
        let probe = Probe {
            a: 1,
            b: 2,
            session_id: [7; 8],
            kind: 1,
        };
        let wire = encode(&Datagram::Probe(probe.clone()));
        assert_eq!(wire.len(), PROBE_LEN);
        assert_eq!(decode(&wire).unwrap(), Datagram::Probe(probe));
        let ack = ProbeAck {
            a: 5,
            b: 6,
            opaque: [8; 60],
        };
        let wire = encode(&Datagram::ProbeAck(ack.clone()));
        assert_eq!(wire.len(), PROBE_ACK_LEN);
        assert_eq!(decode(&wire).unwrap(), Datagram::ProbeAck(ack));
        assert!(decode(&[1, 2, 3, 4]).is_err());
        assert!(decode(&ADVERT.to_le_bytes()).is_err());
    }

    #[test]
    fn advert_field_offsets_match_the_catalogue() {
        let advert = discovery::Advert {
            game_a: 0,
            game_b: 0,
            game_c: 0,
            free_slots: 0,
            name: b"N".to_vec(),
            random_id: [0xAA; 8],
            xnaddr: [0xBB; 36],
            session_id: [0xCC; 8],
            key: [0xDD; 16],
        };
        let wire = discovery::encode(&discovery::Datagram::Advert(advert));
        assert_eq!(wire[50], 0xAA);
        assert_eq!(wire[58], 0xBB);
        assert_eq!(wire[94], 0xCC);
        assert_eq!(wire[102], 0xDD);
        assert_eq!(wire[117], 0xDD);
        let spec_offset = |field| {
            FIELD_CATALOGUE
                .iter()
                .find(|f| f.packet == "discovery_advert(-6)" && f.field == field)
                .unwrap()
                .offset
        };
        assert_eq!(spec_offset("session_id"), Some(94));
    }

    #[test]
    fn fesl_lan_roundtrip() {
        let announce = fesl::Lan::Announce {
            interval_seconds: 30,
            word: 0x0102_0304,
            name: b"tsfp".to_vec(),
            product: b"1.0".to_vec(),
            attributes: b"TCP:~1:1024\tUDP:~1:1024".to_vec(),
            endpoint: b"<private>:1000".to_vec(),
        };
        let wire = fesl::encode(&announce);
        assert_eq!(wire.len(), 384);
        assert_eq!(&wire[..3], b"gEA");
        assert_eq!(&wire[4..8], &[1, 2, 3, 4]);
        assert_eq!(fesl::decode(&wire).unwrap(), announce);
        assert_eq!(
            fesl::decode(&fesl::encode(&fesl::Lan::Query)).unwrap(),
            fesl::Lan::Query
        );
        let clamped = fesl::Lan::Announce {
            interval_seconds: 1,
            word: 0,
            name: b"n".to_vec(),
            product: Vec::new(),
            attributes: Vec::new(),
            endpoint: Vec::new(),
        };
        assert_eq!(fesl::encode(&clamped)[3], 2);
        assert_eq!(fesl::decode(&[0; 384]), Err(CodecError::BadMagic));
    }
}
