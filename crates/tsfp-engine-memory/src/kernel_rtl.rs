// SPDX-License-Identifier: GPL-3.0-or-later
//! T2209 pure kernel runtime model. Contract: docs/t-kernel-rtl-rust.md.
use crate::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

const INVALID_PARAMETER: u32 = 0xc000000d;
const ACCESS_VIOLATION: u32 = 0xc0000005;
const NOT_IMPLEMENTED: u32 = 0xc0000002;
const BUFFER_OVERFLOW: u32 = 0x80000005;
const DAY_TICKS: u64 = 864_000_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CountedString {
    pub length: u16,
    pub capacity: u16,
    pub buffer: GuestAddr,
}
impl CountedString {
    pub fn read(memory: &mut impl GuestMemory, address: u32) -> Result<Self, MemoryFault> {
        let mut bytes = [0; 8];
        memory.read(GuestAddr(address), &mut bytes)?;
        Ok(Self {
            length: u16::from_le_bytes([bytes[0], bytes[1]]),
            capacity: u16::from_le_bytes([bytes[2], bytes[3]]),
            buffer: GuestAddr(u32::from_le_bytes(bytes[4..8].try_into().unwrap())),
        })
    }
    pub fn bytes(self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..2].copy_from_slice(&self.length.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.capacity.to_le_bytes());
        bytes[4..].copy_from_slice(&self.buffer.0.to_le_bytes());
        bytes
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeFields {
    pub year: i16,
    pub month: i16,
    pub day: i16,
    pub hour: i16,
    pub minute: i16,
    pub second: i16,
    pub millisecond: i16,
    pub weekday: i16,
}
impl TimeFields {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        let mut fields = [0; 8];
        for (field, pair) in fields.iter_mut().zip(bytes.chunks_exact(2)) {
            *field = i16::from_le_bytes([pair[0], pair[1]]);
        }
        let [year, month, day, hour, minute, second, millisecond, weekday] = fields;
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            weekday,
        }
    }
    pub fn bytes(self) -> [u8; 16] {
        let mut bytes = [0; 16];
        for (pair, value) in bytes.chunks_exact_mut(2).zip([
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.millisecond,
            self.weekday,
        ]) {
            pair.copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    pub fn to_ticks(self) -> Option<u64> {
        let year = i64::from(self.year);
        if year < 1601
            || !(1..=12).contains(&self.month)
            || self.day < 1
            || !(0..=23).contains(&self.hour)
            || !(0..=59).contains(&self.minute)
            || !(0..=59).contains(&self.second)
            || !(0..=999).contains(&self.millisecond)
        {
            return None;
        }
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let month_days = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        if self.day > month_days[self.month as usize - 1] {
            return None;
        }
        let adjusted_year = year - i64::from(self.month <= 2);
        let era = adjusted_year / 400;
        let yoe = adjusted_year - era * 400;
        let month = i64::from(self.month);
        let phase = if month > 2 { month - 3 } else { month + 9 };
        let doy = (153 * phase + 2) / 5 + i64::from(self.day) - 1;
        let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468 + 134774;
        let seconds =
            (u64::from(self.hour as u16) * 60 + self.minute as u64) * 60 + self.second as u64;
        let ticks =
            days as u64 * DAY_TICKS + seconds * 10_000_000 + self.millisecond as u64 * 10_000;
        (ticks <= i64::MAX as u64).then_some(ticks)
    }
    pub fn from_ticks(ticks: u64) -> Option<Self> {
        if ticks > i64::MAX as u64 {
            return None;
        }
        let days = (ticks / DAY_TICKS) as i64;
        let civil_days = days - 134774 + 719468;
        let era = civil_days / 146097;
        let doe = civil_days - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let phase = (5 * doy + 2) / 153;
        let day = doy - (153 * phase + 2) / 5 + 1;
        let month = if phase < 10 { phase + 3 } else { phase - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        if year > i64::from(i16::MAX) {
            return None;
        }
        let rest = ticks % DAY_TICKS;
        Some(Self {
            year: year as i16,
            month: month as i16,
            day: day as i16,
            hour: (rest / 36_000_000_000) as i16,
            minute: (rest / 600_000_000 % 60) as i16,
            second: (rest / 10_000_000 % 60) as i16,
            millisecond: (rest / 10_000 % 1000) as i16,
            weekday: ((days + 1) % 7) as i16,
        })
    }
}

/// Diagnostic state belongs to the Rust runtime, never to guest memory.
#[derive(Debug, Default)]
pub struct KernelRtl {
    pub truncated: u32,
    pub nonascii_folds: u32,
    pub last_nonascii: u8,
    pub nonascii_conversions: u32,
    pub compare_refused: u32,
    pub time_refused: u32,
    pub unmapped: u32,
    pub last_unmapped: u32,
}
impl KernelRtl {
    pub fn status_to_dos(&mut self, status: u32) -> u32 {
        let mapped = match status {
            0 => 0,
            0xc0000008 => 6,
            0xc0000017 => 8,
            0xc000000d | 0xc0000003 => 87,
            0x103 => 997,
            0xc0000034 | 0xc000000f => 2,
            0x80000005 => 234,
            0x80000006 => 18,
            0xc0000004 => 24,
            0xc0000010 => 1,
            0xc0000011 => 38,
            0xc0000022 | 0xc00000ba => 5,
            0xc0000023 => 122,
            0xc0000024 => 6,
            0xc0000033 => 123,
            0xc0000035 => 183,
            0xc000003a => 3,
            0xc0000043 => 32,
            0xc000007f => 112,
            0xc000009a => 1450,
            0xc0000101 => 145,
            0xc0000103 => 267,
            0xc000014f => 1005,
            _ => {
                self.unmapped = self.unmapped.wrapping_add(1);
                self.last_unmapped = status;
                317
            }
        };
        mapped
    }
    pub fn equal(
        &mut self,
        memory: &mut impl GuestMemory,
        first: u32,
        second: u32,
        insensitive: bool,
    ) -> Result<bool, MemoryFault> {
        let first = CountedString::read(memory, first)?;
        let second = CountedString::read(memory, second)?;
        if first.length != second.length {
            return Ok(false);
        }
        if first.length == 0 {
            return Ok(true);
        }
        let mut left = vec![0; first.length as usize];
        let mut right = vec![0; second.length as usize];
        memory.read(first.buffer, &mut left)?;
        memory.read(second.buffer, &mut right)?;
        let mut offender = None;
        let mut equal = true;
        for (left, right) in left.into_iter().zip(right) {
            if insensitive && offender.is_none() && (left >= 128 || right >= 128) {
                offender = Some(if left >= 128 { left } else { right });
            }
            equal &= if insensitive {
                left.to_ascii_uppercase() == right.to_ascii_uppercase()
            } else {
                left == right
            };
        }
        if let Some(byte) = offender {
            self.nonascii_folds = self.nonascii_folds.wrapping_add(1);
            self.last_nonascii = byte;
        }
        Ok(equal)
    }
    pub fn init_ansi(
        &mut self,
        memory: &mut impl GuestMemory,
        destination: u32,
        source: u32,
    ) -> Result<(), MemoryFault> {
        // Validate the destination before scanning; the C handler also probes it first.
        let _ = CountedString::read(memory, destination)?;
        let mut descriptor = CountedString::default();
        if source != 0 {
            let mut length = 0u32;
            let mut terminated = false;
            while length <= 65534 {
                let mut byte = [0];
                if memory
                    .read(GuestAddr(source.wrapping_add(length)), &mut byte)
                    .is_err()
                {
                    break;
                }
                if byte[0] == 0 {
                    terminated = true;
                    break;
                }
                length += 1;
            }
            if !terminated {
                self.truncated = self.truncated.wrapping_add(1);
            }
            length = length.min(65534);
            descriptor = CountedString {
                length: length as u16,
                capacity: (length + 1) as u16,
                buffer: GuestAddr(source),
            };
        }
        memory.write(GuestAddr(destination), &descriptor.bytes())
    }
    pub fn convert(
        &mut self,
        memory: &mut impl GuestMemory,
        destination: u32,
        source: u32,
        allocate: u32,
        widening: bool,
    ) -> u32 {
        if allocate as u8 != 0 {
            return NOT_IMPLEMENTED;
        }
        let Ok(source) = CountedString::read(memory, source) else {
            return ACCESS_VIOLATION;
        };
        let Ok(target) = CountedString::read(memory, destination) else {
            return ACCESS_VIOLATION;
        };
        if !widening && source.length % 2 != 0 {
            return INVALID_PARAMETER;
        }
        let characters = if widening {
            source.length as usize
        } else {
            source.length as usize / 2
        };
        let converted = characters * if widening { 2 } else { 1 };
        let needed = converted + if widening { 2 } else { 1 };
        if needed > 65535 {
            return INVALID_PARAMETER;
        }
        if usize::from(target.capacity) < needed {
            return BUFFER_OVERFLOW;
        }
        let mut input = vec![0; source.length as usize];
        if !input.is_empty() && memory.read(source.buffer, &mut input).is_err() {
            return ACCESS_VIOLATION;
        }
        let mut output = vec![0; needed];
        let mut replaced = 0u32;
        if widening {
            for (index, byte) in input.into_iter().enumerate() {
                output[index * 2] = byte;
                replaced += u32::from(byte >= 128);
            }
        } else {
            for (index, pair) in input.chunks_exact(2).enumerate() {
                let wide = u16::from_le_bytes([pair[0], pair[1]]);
                output[index] = if wide <= 255 { wide as u8 } else { b'?' };
                replaced += u32::from(wide > 255);
            }
        }
        if memory.write(target.buffer, &output).is_err()
            || memory
                .write(GuestAddr(destination), &(converted as u16).to_le_bytes())
                .is_err()
        {
            return ACCESS_VIOLATION;
        }
        self.nonascii_conversions = self.nonascii_conversions.wrapping_add(replaced);
        0
    }
    /// Validate every whole-word byte, including bytes after the first mismatch.
    /// Fixed-size chunks avoid allocating a guest-controlled, potentially 4GB Vec.
    pub fn compare_ulong(
        &mut self,
        memory: &mut impl GuestMemory,
        source: u32,
        length: u32,
        pattern: u32,
    ) -> u32 {
        let length = length / 4 * 4;
        if u64::from(source) + u64::from(length) > 1u64 << 32 {
            self.compare_refused = self.compare_refused.wrapping_add(1);
            return 0;
        }
        let mut chunk = [0; 4096];
        let mut offset = 0u32;
        let mut matched = 0u32;
        let mut prefix = true;
        while offset < length {
            let count = (length - offset).min(chunk.len() as u32) as usize;
            let read = source
                .checked_add(offset)
                .and_then(|address| memory.read(GuestAddr(address), &mut chunk[..count]).ok());
            if read.is_none() {
                self.compare_refused = self.compare_refused.wrapping_add(1);
                return 0;
            }
            for word in chunk[..count].chunks_exact(4) {
                prefix &= u32::from_le_bytes(word.try_into().unwrap()) == pattern;
                if prefix {
                    matched += 4;
                }
            }
            offset += count as u32;
        }
        matched
    }
    /// Returns None for unsupported ordinals; callers retain C for those.
    pub fn dispatch(
        &mut self,
        memory: &mut impl GuestMemory,
        ordinal: u32,
        args: &[u32],
    ) -> Option<u32> {
        let arity = match ordinal {
            301 => 1,
            289 | 304 | 305 => 2,
            260 | 269 | 279 | 308 => 3,
            _ => return None,
        };
        if args.len() < arity {
            return None;
        }
        let result = match ordinal {
            301 => self.status_to_dos(args[0]),
            289 => {
                let _ = self.init_ansi(memory, args[0], args[1]);
                0
            }
            279 => u32::from(
                self.equal(memory, args[0], args[1], args[2] != 0)
                    .unwrap_or(false),
            ),
            260 | 308 => self.convert(memory, args[0], args[1], args[2], ordinal == 260),
            269 => self.compare_ulong(memory, args[0], args[1], args[2]),
            304 | 305 => {
                let mut input = vec![0; if ordinal == 304 { 16 } else { 8 }];
                let mut probe = vec![0; if ordinal == 304 { 8 } else { 16 }];
                let outcome = (|| {
                    memory.read(GuestAddr(args[0]), &mut input).ok()?;
                    memory.read(GuestAddr(args[1]), &mut probe).ok()?;
                    let output = if ordinal == 304 {
                        TimeFields::from_bytes(input.try_into().unwrap())
                            .to_ticks()?
                            .to_le_bytes()
                            .to_vec()
                    } else {
                        TimeFields::from_ticks(u64::from_le_bytes(input.try_into().unwrap()))?
                            .bytes()
                            .to_vec()
                    };
                    memory.write(GuestAddr(args[1]), &output).ok()?;
                    Some(u32::from(ordinal == 304))
                })();
                outcome.unwrap_or_else(|| {
                    self.time_refused = self.time_refused.wrapping_add(1);
                    0
                })
            }
            _ => unreachable!(),
        };
        Some(result)
    }
}
