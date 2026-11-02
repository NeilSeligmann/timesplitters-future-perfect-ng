// SPDX-License-Identifier: GPL-3.0-or-later
//! One route condition (`route_cond`) and the memory condition grammar `[*]ADDR[+OFF][:W]OP VALUE[&MASK]`.
use crate::COND_TEXT;
use crate::number::number;

pub struct CondKind;
impl CondKind {
    pub const MEM: u32 = 1;
    pub const FILE_OPEN: u32 = 2;
    pub const FILE_READ: u32 = 3;
    pub const FILE_IDLE: u32 = 4;
    pub const CALL: u32 = 5;
    pub const FRAME_CHANGE: u32 = 6;
    pub const FRAME_STABLE: u32 = 7;
}

pub struct Cmp;
impl Cmp {
    pub const EQ: u32 = 1;
    pub const NE: u32 = 2;
    pub const LT: u32 = 3;
    pub const LE: u32 = 4;
    pub const GT: u32 = 5;
    pub const GE: u32 = 6;
}

/// `route_cond` of `xinput_route_spec.h`, same layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cond {
    pub kind: u32,
    pub indirect: bool,
    pub address: u32,
    pub offset: u32,
    pub width: u32,
    pub cmp: u32,
    pub value: u32,
    pub mask: u32,
    pub text: [u8; COND_TEXT],
    pub va: u32,
    pub amount: u64,
}

impl Cond {
    pub const ZERO: Cond = Cond {
        kind: 0,
        indirect: false,
        address: 0,
        offset: 0,
        width: 0,
        cmp: 0,
        value: 0,
        mask: 0,
        text: [0; COND_TEXT],
        va: 0,
        amount: 0,
    };

    pub fn set_text(&mut self, text: &[u8]) {
        self.text = [0; COND_TEXT];
        self.text[..text.len()].copy_from_slice(text);
    }

    pub fn text_bytes(&self) -> &[u8] {
        let end = self
            .text
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(COND_TEXT);
        &self.text[..end]
    }
}

pub(crate) const MEM_END: u64 = 0x0400_0000;

/// Lower-case file substring: 1..47 printable ASCII bytes, no comma.
pub(crate) fn lower_text(text: &[u8]) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() >= COND_TEXT {
        return None;
    }
    if !text
        .iter()
        .all(|&byte| (0x20..=0x7E).contains(&byte) && byte != b',')
    {
        return None;
    }
    Some(text.to_ascii_lowercase())
}

fn find(haystack: &[u8], needle: u8) -> Option<usize> {
    haystack.iter().position(|&byte| byte == needle)
}

/// Value and optional `&MASK` after the comparison operator.
fn value_and_mask(rest: &[u8]) -> Option<(u64, u64)> {
    let amp = find(rest, b'&');
    let value = number(&rest[..amp.unwrap_or(rest.len())], 0xFFFF_FFFF)?;
    let mask = match amp {
        Some(at) => number(&rest[at + 1..], 0xFFFF_FFFF)?,
        None => 0xFFFF_FFFF,
    };
    Some((value, mask))
}

/// `mem=` of the event grammar (text after `mem=`).
pub fn mem_event_condition(text: &[u8]) -> Option<Cond> {
    if text.is_empty() || text.len() >= 96 {
        return None;
    }
    let indirect = text[0] == b'*';
    let start = &text[usize::from(indirect)..];
    let (op_at, op_len, cmp) = (0..start.len()).find_map(|at| {
        let pair = (start[at], start.get(at + 1).copied());
        match pair {
            (b'=', Some(b'=')) => Some((at, 2, Cmp::EQ)),
            (b'!', Some(b'=')) => Some((at, 2, Cmp::NE)),
            (b'<', Some(b'=')) => Some((at, 2, Cmp::LE)),
            (b'>', Some(b'=')) => Some((at, 2, Cmp::GE)),
            (b'<', _) => Some((at, 1, Cmp::LT)),
            (b'>', _) => Some((at, 1, Cmp::GT)),
            _ => None,
        }
    })?;
    let colon = find(&start[..op_at], b':');
    let mut address_end = colon.unwrap_or(op_at);
    let mut offset = 0;
    if indirect && let Some(plus) = find(&start[..address_end], b'+') {
        offset = number(&start[plus + 1..address_end], 0xFFFF_FFFF)?;
        address_end = plus;
    }
    let address = number(&start[..address_end], 0xFFFF_FFFF)?;
    let mut width = 4;
    if let Some(at) = colon {
        width = number(&start[at + 1..op_at], 4)?;
        if !matches!(width, 1 | 2 | 4) {
            return None;
        }
    }
    let (value, mut mask) = value_and_mask(&start[op_at + op_len..])?;
    if width < 4 {
        mask &= (1u64 << (8 * width)) - 1;
    }
    if indirect {
        if address % 4 != 0 || address + 4 > MEM_END {
            return None;
        }
    } else if address % width != 0 || address + width > MEM_END {
        return None;
    }
    if cmp == Cmp::EQ && value & !mask != 0 {
        return None;
    }
    let mut cond = Cond::ZERO;
    cond.kind = CondKind::MEM;
    cond.indirect = indirect;
    cond.address = address as u32;
    cond.offset = offset as u32;
    cond.width = width as u32;
    cond.cmp = cmp;
    cond.value = value as u32;
    cond.mask = mask as u32;
    Some(cond)
}

/// `[*]ADDR[+OFF][:W][&MASK]` without a comparison, parsed as `==0`.
pub fn mem_ref(text: &[u8]) -> Option<Cond> {
    if text.iter().any(|byte| b"=<>!".contains(byte)) {
        return None;
    }
    let amp = find(text, b'&');
    let head = amp.unwrap_or(text.len());
    let tail = amp.map_or(0, |at| text.len() - at);
    if head == 0 || head + 3 + tail >= 96 {
        return None;
    }
    let mut spec = text[..head].to_vec();
    spec.extend_from_slice(b"==0");
    spec.extend_from_slice(&text[head..]);
    mem_event_condition(&spec)
}

/// The legacy `ADDR[:W]==VALUE[&MASK]` of the plain `--route-wait` grammar: (address, width, value, mask).
pub fn legacy_mem(text: &[u8]) -> Option<(u32, u32, u32, u32)> {
    let eq = text.windows(2).position(|pair| pair == b"==")?;
    let colon = find(&text[..eq], b':');
    let address = number(&text[..colon.unwrap_or(eq)], 0xFFFF_FFFF)?;
    let mut width = 4;
    if let Some(at) = colon {
        width = number(&text[at + 1..eq], 4)?;
        if !matches!(width, 1 | 2 | 4) {
            return None;
        }
    }
    let (value, mut mask) = value_and_mask(&text[eq + 2..])?;
    if address % width != 0 || address + width > MEM_END {
        return None;
    }
    if width < 4 {
        mask &= (1u64 << (8 * width)) - 1;
    }
    if value & !mask != 0 {
        return None;
    }
    Some((address as u32, width as u32, value as u32, mask as u32))
}

pub fn compare(cmp: u32, left: u32, right: u32) -> bool {
    match cmp {
        Cmp::EQ => left == right,
        Cmp::NE => left != right,
        Cmp::LT => left < right,
        Cmp::LE => left <= right,
        Cmp::GT => left > right,
        Cmp::GE => left >= right,
        _ => false,
    }
}

/// Short log text of one condition, the same string `xinput_route_cond_format` builds.
pub fn format(cond: &Cond) -> String {
    let op = match cond.cmp {
        Cmp::EQ => "==",
        Cmp::NE => "!=",
        Cmp::LT => "<",
        Cmp::LE => "<=",
        Cmp::GT => ">",
        Cmp::GE => ">=",
        _ => "",
    };
    let text = String::from_utf8_lossy(cond.text_bytes());
    match cond.kind {
        CondKind::MEM if cond.indirect => {
            format!(
                "mem=*0x{:X}+0x{:X}:{}{op}0x{:X}",
                cond.address, cond.offset, cond.width, cond.value
            )
        }
        CondKind::MEM => format!(
            "mem=0x{:X}:{}{op}0x{:X}",
            cond.address, cond.width, cond.value
        ),
        CondKind::FILE_OPEN => format!("file-open={text}"),
        CondKind::FILE_READ => format!("file-read={text}@{}", cond.amount),
        CondKind::FILE_IDLE if !text.is_empty() => format!("file-idle={}@{text}", cond.amount),
        CondKind::FILE_IDLE => format!("file-idle={}", cond.amount),
        CondKind::CALL => format!("call=0x{:X}@{}", cond.va, cond.amount),
        CondKind::FRAME_CHANGE => "frame-change".to_string(),
        CondKind::FRAME_STABLE => format!("frame-stable={}", cond.amount),
        _ => String::new(),
    }
}
