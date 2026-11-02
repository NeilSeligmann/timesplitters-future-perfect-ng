// SPDX-License-Identifier: GPL-3.0-or-later
//! T3741 spool format (docs/t3738-capture-design.md "Spool format"), read side plus an encoder
//! for synthetic test corpora. Byte-compatible with `tools/t2corpus/spool.py`: magic
//! `T2CAP\x01`, u32 header length, JSON header, then u32-length-prefixed records until EOF. A
//! truncated final record is dropped and counted. Without the magic the input is read as a raw
//! T3741 shard (`<va>/entries.bin`: the same records back to back, no length prefix).
use tsfp_port_runtime::abi::{ExtRegisters, Fpu, Xmm};

pub const MAGIC: &[u8] = b"T2CAP\x01";
pub const REGS_SIZE: usize = 240;

/// Flag bits of a record (capture_incomplete reasons), as in spool.py `FLAG_BITS`.
pub const FLAG_NAMES: [(&str, u32); 8] = [
    ("mmio", 1),
    ("thread_switch", 2),
    ("size", 4),
    ("seh_unwind", 8),
    ("fpu_overflow", 16),
    ("self_modifying", 32),
    // T3740: synthetic, a format 1 record (no implicit thread state): skipped, recapture it.
    ("old_format", OLD_FORMAT),
    // T3767: host code changed a byte the frame knew, with no callee to attribute it to.
    ("host_write", 128),
];
pub const OLD_FORMAT: u32 = 64;
/// T3740 record format 2: raw flags bit saying the implicit-state tail is present.
pub const FORMAT_IMPLICIT: u32 = 0x8000_0000;
pub const FORMAT_CLOCK_EVENTS: u32 = 0x4000_0000;
pub const IMPLICIT_SIZE: usize = 32;
/// Hard ceiling of one record (T3836, = tools/t2corpus/spool.py MAX_RECORD_BYTES).
pub const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;

/// Implicit thread state the lifted code reads outside the register file (src/host/t2_capture.c
/// `snapshot_implicit`, spool.py `IMPLICIT_NAMES`): words in this order.
pub type Implicit = [u32; 8];
pub const IMPLICIT_NAMES: [&str; 8] = [
    "seh_ebp",
    "df",
    "fp_cmp",
    "fp_cc",
    "flag_bridge_eflags",
    "flag_bridge_mask",
    "fs_base",
    "reserved",
];

/// capture_incomplete bits of a raw flags word: the format bit stripped, `old_format` added.
pub fn effective_flags(raw: u32) -> u32 {
    let flags = raw & !(FORMAT_IMPLICIT | FORMAT_CLOCK_EVENTS);
    if raw & FORMAT_IMPLICIT != 0 {
        flags
    } else {
        flags | OLD_FORMAT
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub addr: u32,
    pub data: Vec<u8>,
}

/// A direct clock read, ordered relative to the root's immediate callees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockEvent {
    pub after_callees: u32,
    pub value: u64,
    pub writes: Vec<Run>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalleeRecord {
    pub va: u32,
    pub entry_regs: ExtRegisters,
    pub args: Vec<u32>,
    pub exit_regs: ExtRegisters,
    pub write_runs: Vec<Run>,
    /// Implicit state after the call returned (format 2).
    pub exit_implicit: Option<Implicit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub va: u32,
    pub caller_va: u32,
    pub return_address: u32,
    pub poll: u32,
    pub guest_thread: u32,
    /// capture_incomplete bits (`effective_flags` of the raw word).
    pub flags: u32,
    /// The raw 240 entry-register bytes: the input digest hashes them as captured.
    pub entry_raw: [u8; REGS_SIZE],
    pub entry_regs: ExtRegisters,
    pub exit_regs: ExtRegisters,
    pub read_runs: Vec<Run>,
    pub write_runs: Vec<Run>,
    pub callees: Vec<CalleeRecord>,
    /// Format 2 implicit thread state at entry and exit (None: format 1, flagged `old_format`).
    pub entry_implicit: Option<Implicit>,
    pub exit_implicit: Option<Implicit>,
    pub clock_events: Option<Vec<ClockEvent>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpoolError(pub String);

impl core::fmt::Display for SpoolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(text: &str) -> Result<T, SpoolError> {
    Err(SpoolError(text.to_string()))
}

/// The register file of the `ExtRegisters` ABI from its 240 little-endian bytes.
pub fn decode_regs(raw: &[u8]) -> ExtRegisters {
    assert_eq!(raw.len(), REGS_SIZE, "register blob size");
    let word = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().expect("4 bytes"));
    let lanes = |at: usize| Xmm {
        lanes: [word(at), word(at + 4), word(at + 8), word(at + 12)],
    };
    let mut stack = [0f64; 8];
    for (slot, value) in stack.iter_mut().enumerate() {
        let at = 40 + slot * 8;
        *value = f64::from_le_bytes(raw[at..at + 8].try_into().expect("8 bytes"));
    }
    ExtRegisters {
        eax: word(0),
        ecx: word(4),
        edx: word(8),
        esp: word(12),
        ebx: word(16),
        ebp: word(20),
        esi: word(24),
        edi: word(28),
        eflags: word(32),
        fpu: Fpu {
            stack,
            top: word(104),
            control: u16::from_le_bytes([raw[108], raw[109]]),
        },
        xmm: core::array::from_fn(|i| lanes(112 + 16 * i)),
        xmm_hi: core::array::from_fn(|i| lanes(192 + 16 * i)),
    }
}

/// The 240-byte image of a register file (padding bytes zero).
pub fn encode_regs(regs: &ExtRegisters) -> [u8; REGS_SIZE] {
    let mut raw = [0u8; REGS_SIZE];
    let gprs = [
        regs.eax,
        regs.ecx,
        regs.edx,
        regs.esp,
        regs.ebx,
        regs.ebp,
        regs.esi,
        regs.edi,
        regs.eflags,
    ];
    for (i, value) in gprs.iter().enumerate() {
        raw[4 * i..4 * i + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (slot, value) in regs.fpu.stack.iter().enumerate() {
        raw[40 + 8 * slot..48 + 8 * slot].copy_from_slice(&value.to_le_bytes());
    }
    raw[104..108].copy_from_slice(&regs.fpu.top.to_le_bytes());
    raw[108..110].copy_from_slice(&regs.fpu.control.to_le_bytes());
    for (i, xmm) in regs.xmm.iter().chain(regs.xmm_hi.iter()).enumerate() {
        for (lane, value) in xmm.lanes.iter().enumerate() {
            let at = 112 + 16 * i + 4 * lane;
            raw[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    raw
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], SpoolError> {
        let end = self
            .pos
            .checked_add(size)
            .filter(|&end| end <= self.data.len());
        let Some(end) = end else {
            return err("record overruns its length");
        };
        let chunk = &self.data[self.pos..end];
        self.pos = end;
        Ok(chunk)
    }
    fn u32(&mut self) -> Result<u32, SpoolError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    fn implicit(&mut self) -> Result<Implicit, SpoolError> {
        let raw = self.take(IMPLICIT_SIZE)?;
        Ok(core::array::from_fn(|i| {
            u32::from_le_bytes(raw[4 * i..4 * i + 4].try_into().expect("4 bytes"))
        }))
    }
    fn regs(&mut self) -> Result<(&'a [u8], ExtRegisters), SpoolError> {
        let raw = self.take(REGS_SIZE)?;
        Ok((raw, decode_regs(raw)))
    }
    fn runs(&mut self) -> Result<Vec<Run>, SpoolError> {
        let count = self.u32()?;
        let mut runs = Vec::new();
        for _ in 0..count {
            let addr = self.u32()?;
            let len = self.u32()? as usize;
            runs.push(Run {
                addr,
                data: self.take(len)?.to_vec(),
            });
        }
        Ok(runs)
    }
    fn entry(&mut self) -> Result<Entry, SpoolError> {
        let va = self.u32()?;
        let caller_va = self.u32()?;
        let return_address = self.u32()?;
        let poll = self.u32()?;
        let guest_thread = self.u32()?;
        let flags = self.u32()?;
        let (raw, entry_regs) = self.regs()?;
        let (_, exit_regs) = self.regs()?;
        let read_runs = self.runs()?;
        let write_runs = self.runs()?;
        let mut callees = Vec::new();
        for _ in 0..self.u32()? {
            let va = self.u32()?;
            let (_, entry_regs) = self.regs()?;
            let n_args = self.u32()?;
            let args = (0..n_args).map(|_| self.u32()).collect::<Result<_, _>>()?;
            let (_, exit_regs) = self.regs()?;
            let write_runs = self.runs()?;
            callees.push(CalleeRecord {
                va,
                entry_regs,
                args,
                exit_regs,
                write_runs,
                exit_implicit: None,
            });
        }
        let (mut entry_implicit, mut exit_implicit) = (None, None);
        if flags & FORMAT_IMPLICIT != 0 {
            entry_implicit = Some(self.implicit()?);
            exit_implicit = Some(self.implicit()?);
            for callee in &mut callees {
                callee.exit_implicit = Some(self.implicit()?);
            }
        }
        let clock_events = if flags & FORMAT_CLOCK_EVENTS != 0 {
            if flags & FORMAT_IMPLICIT == 0 {
                return err("clock events require implicit state");
            }
            let count = self.u32()? as usize;
            if count > (self.data.len() - self.pos) / 16 {
                return err("clock event count exceeds record");
            }
            let mut events = Vec::new();
            let mut previous = 0;
            for _ in 0..count {
                let after_callees = self.u32()?;
                if after_callees < previous || after_callees as usize > callees.len() {
                    return err("invalid clock event order");
                }
                previous = after_callees;
                let low = self.u32()? as u64;
                let high = self.u32()? as u64;
                events.push(ClockEvent {
                    after_callees,
                    value: low | (high << 32),
                    writes: self.runs()?,
                });
            }
            Some(events)
        } else {
            None
        };
        Ok(Entry {
            va,
            caller_va,
            return_address,
            poll,
            guest_thread,
            flags: effective_flags(flags),
            entry_raw: raw.try_into().expect("240 bytes"),
            entry_regs,
            exit_regs,
            read_runs,
            write_runs,
            callees,
            entry_implicit,
            exit_implicit,
            clock_events,
        })
    }
}

/// One record body (`decode_record` of spool.py: trailing bytes are an error).
pub fn decode_record(data: &[u8]) -> Result<Entry, SpoolError> {
    let mut cursor = Cursor { data, pos: 0 };
    let entry = cursor.entry()?;
    if cursor.pos != data.len() {
        return err("trailing bytes in record");
    }
    Ok(entry)
}

fn put_runs(out: &mut Vec<u8>, runs: &[Run]) {
    out.extend((runs.len() as u32).to_le_bytes());
    for run in runs {
        out.extend(run.addr.to_le_bytes());
        out.extend((run.data.len() as u32).to_le_bytes());
        out.extend(&run.data);
    }
}

/// `encode_record` of spool.py. The entry registers are written from `entry_raw`.
pub fn encode_record(entry: &Entry) -> Vec<u8> {
    let mut out = Vec::new();
    let v2 = entry.entry_implicit.is_some();
    assert!(
        entry.clock_events.is_none() || v2,
        "clock events require implicit state"
    );
    let flags = (entry.flags & !OLD_FORMAT)
        | if v2 { FORMAT_IMPLICIT } else { 0 }
        | if entry.clock_events.is_some() {
            FORMAT_CLOCK_EVENTS
        } else {
            0
        };
    for word in [
        entry.va,
        entry.caller_va,
        entry.return_address,
        entry.poll,
        entry.guest_thread,
        flags,
    ] {
        out.extend(word.to_le_bytes());
    }
    out.extend(entry.entry_raw);
    out.extend(encode_regs(&entry.exit_regs));
    put_runs(&mut out, &entry.read_runs);
    put_runs(&mut out, &entry.write_runs);
    out.extend((entry.callees.len() as u32).to_le_bytes());
    for callee in &entry.callees {
        out.extend(callee.va.to_le_bytes());
        out.extend(encode_regs(&callee.entry_regs));
        out.extend((callee.args.len() as u32).to_le_bytes());
        for arg in &callee.args {
            out.extend(arg.to_le_bytes());
        }
        out.extend(encode_regs(&callee.exit_regs));
        put_runs(&mut out, &callee.write_runs);
    }
    if let Some(entry_implicit) = entry.entry_implicit {
        let blocks = [entry_implicit, entry.exit_implicit.unwrap_or_default()]
            .into_iter()
            .chain(
                entry
                    .callees
                    .iter()
                    .map(|c| c.exit_implicit.unwrap_or_default()),
            );
        for block in blocks {
            for word in block {
                out.extend(word.to_le_bytes());
            }
        }
    }
    if let Some(events) = &entry.clock_events {
        out.extend((events.len() as u32).to_le_bytes());
        let mut previous = 0;
        for event in events {
            assert!(
                event.after_callees >= previous
                    && event.after_callees as usize <= entry.callees.len()
            );
            previous = event.after_callees;
            out.extend(event.after_callees.to_le_bytes());
            out.extend(event.value.to_le_bytes());
            put_runs(&mut out, &event.writes);
        }
    }
    out
}

/// A spool file image (`write_spool` of spool.py) with a JSON header.
pub fn encode_spool(header_json: &str, records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend((header_json.len() as u32).to_le_bytes());
    out.extend(header_json.as_bytes());
    for record in records {
        out.extend((record.len() as u32).to_le_bytes());
        out.extend(record);
    }
    out
}

/// A parsed input: header text (empty for a raw shard) and one result per record in order.
#[derive(Debug)]
pub struct Spool {
    pub header: String,
    pub records: Vec<Result<Entry, SpoolError>>,
    /// 1 when a partial final record was dropped.
    pub truncated: u32,
}

impl Spool {
    /// A string value of the JSON header (flat keys only, enough for the T3741 header).
    pub fn header_value(&self, key: &str) -> Option<String> {
        let pattern = format!("\"{key}\":");
        let at = self.header.find(&pattern)? + pattern.len();
        let rest = self.header[at..].trim_start().strip_prefix('"')?;
        Some(rest[..rest.find('"')?].to_string())
    }
}

pub fn parse(data: &[u8]) -> Result<Spool, SpoolError> {
    if !data.starts_with(MAGIC) {
        return Ok(parse_shard(data));
    }
    let mut pos = MAGIC.len();
    let Some(len) = data.get(pos..pos + 4) else {
        return err("truncated header length");
    };
    let header_len = u32::from_le_bytes(len.try_into().expect("4 bytes")) as usize;
    pos += 4;
    let Some(header) = data.get(pos..pos + header_len) else {
        return err("truncated header");
    };
    let Ok(header) = core::str::from_utf8(header) else {
        return err("header is not UTF-8");
    };
    pos += header_len;
    let mut records = Vec::new();
    let mut truncated = 0;
    while pos < data.len() {
        let Some(len) = data.get(pos..pos + 4) else {
            truncated = 1;
            break;
        };
        let size = u32::from_le_bytes(len.try_into().expect("4 bytes")) as usize;
        if size > MAX_RECORD_BYTES {
            truncated = 1; // T3836: a corrupt length word, the rest cannot be framed
            break;
        }
        let Some(body) = data.get(pos + 4..pos + 4 + size) else {
            truncated = 1;
            break;
        };
        records.push(decode_record(body));
        pos += 4 + size;
    }
    Ok(Spool {
        header: header.to_string(),
        records,
        truncated,
    })
}

/// A raw T3741 shard: records back to back. A record that does not parse ends the shard
/// (the rest cannot be framed) and counts as truncated.
fn parse_shard(data: &[u8]) -> Spool {
    let mut cursor = Cursor { data, pos: 0 };
    let mut records = Vec::new();
    let mut truncated = 0;
    while cursor.pos < data.len() {
        match cursor.entry() {
            Ok(entry) => records.push(Ok(entry)),
            Err(_) => {
                truncated = 1;
                break;
            }
        }
    }
    Spool {
        header: String::new(),
        records,
        truncated,
    }
}

pub fn flag_names(flags: u32) -> Vec<&'static str> {
    FLAG_NAMES
        .iter()
        .filter(|(_, bit)| flags & bit != 0)
        .map(|(name, _)| *name)
        .collect()
}
