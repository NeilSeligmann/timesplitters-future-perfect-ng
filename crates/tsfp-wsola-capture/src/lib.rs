// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline comparison of a sealed original host WSOLA engine event stream.
use std::io::Read;
use tsfp_audio::stretch::Stretch;

const LIMIT: usize = 256 * 1024 * 1024;
const INIT: u64 = 1469598103934665603;

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub calls: u64,
    pub primes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Prime,
    Next,
    Hold,
    Leave,
    Fade,
}

impl TryFrom<u8> for Operation {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Prime),
            2 => Ok(Self::Next),
            3 => Ok(Self::Hold),
            4 => Ok(Self::Leave),
            5 => Ok(Self::Fade),
            _ => Err("unknown operation"),
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.offset.checked_add(n).ok_or("length overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated capture")?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn samples(&mut self, n: usize) -> Result<Vec<i16>, String> {
        let data = self.take(n.checked_mul(2).ok_or("sample length overflow")?)?;
        Ok(data
            .chunks_exact(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect())
    }
}

/// Reject gaps, truncation, extra bytes, oversized captures and no successful primes.
/// A successful result compares Rust against the captured original C result for every call.
pub fn compare(mut source: impl Read) -> Result<Report, String> {
    replay(&mut source, false, false)
}

/// Diagnostic only: compare the complete ordered prefix of an unsealed stream.
/// Success is not a conformance verdict and never implies capture completeness.
pub fn audit_prefix(mut source: impl Read) -> Result<Report, String> {
    replay(&mut source, true, false)
}

/// Compare a sealed capture through the native sink backend C ABI.
pub fn compare_native_abi(mut source: impl Read) -> Result<Report, String> {
    replay(&mut source, false, true)
}

struct NativeAbi(*mut tsfp_wsola_backend::Engine);
impl Drop for NativeAbi {
    fn drop(&mut self) {
        unsafe { tsfp_wsola_backend::tsfp_wsola_destroy(self.0) }
    }
}

fn read_bounded(mut source: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    source
        .by_ref()
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "capture read failed")?;
    if bytes.len() > limit {
        return Err("capture exceeds bound".into());
    }
    Ok(bytes)
}

fn replay(source: impl Read, audit: bool, native_abi: bool) -> Result<Report, String> {
    let bytes = read_bounded(source, LIMIT)?;
    let mut c = Cursor {
        bytes: &bytes,
        offset: 0,
    };
    if c.take(8)? != b"WSOLAC1\n" {
        return Err("bad capture header".into());
    }
    let channels = u16::try_from(c.u64()?).map_err(|_| "invalid channels")?;
    let overlap = usize::try_from(c.u64()?).map_err(|_| "invalid overlap")?;
    let block = overlap
        .checked_mul(usize::from(channels))
        .ok_or("invalid geometry")?;
    if block > LIMIT / 2 {
        return Err("invalid geometry".into());
    }
    let mut engine = if native_abi {
        None
    } else {
        Some(Stretch::new(channels, overlap).ok_or("invalid geometry")?)
    };
    let abi = if native_abi {
        let ptr = unsafe {
            tsfp_wsola_backend::tsfp_wsola_create(1, channels, overlap, std::ptr::null())
        };
        if ptr.is_null() {
            return Err("native backend create failed".into());
        }
        Some(NativeAbi(ptr))
    } else {
        None
    };
    let mut report = Report {
        calls: 0,
        primes: 0,
    };
    loop {
        if audit && c.offset == bytes.len() {
            return Ok(report);
        }
        let start = c.offset;
        let op = c.byte()?;
        if op == 255 {
            if audit && bytes.len() - c.offset < 16 {
                return Ok(report);
            }
            let count = c.u64()?;
            let expected_hash = c.u64()?;
            let hash = bytes[..start]
                .iter()
                .fold(INIT, |h, b| (h ^ u64::from(*b)).wrapping_mul(1099511628211));
            if c.offset != bytes.len() || count != report.calls || hash != expected_hash {
                return Err("invalid capture seal".into());
            }
            if report.primes == 0 {
                return Err("no successful prime calls".into());
            }
            return Ok(report);
        }
        let op = Operation::try_from(op)?;
        if audit && bytes.len() - c.offset < 26 {
            return Ok(report);
        }
        let frames = usize::try_from(c.u64()?).map_err(|_| "invalid frames")?;
        let nominal = usize::try_from(c.u64()?).map_err(|_| "invalid nominal")?;
        let ok = match c.byte()? {
            0 => false,
            1 => true,
            _ => return Err("invalid result".into()),
        };
        let chosen = usize::try_from(c.u64()?).map_err(|_| "invalid chosen offset")?;
        let primed = match c.byte()? {
            0 => false,
            1 => true,
            _ => return Err("invalid primed state".into()),
        };
        if (op != Operation::Next && (nominal != 0 || chosen != 0))
            || (!ok && chosen != 0)
            || matches!(op, Operation::Hold | Operation::Fade) && frames != 0
        {
            return Err("invalid operation fields".into());
        }
        let samples = frames
            .checked_mul(usize::from(channels))
            .ok_or("input overflow")?;
        if samples > LIMIT / 2 {
            return Err("input exceeds bound".into());
        }
        let input_payload =
            if ok && matches!(op, Operation::Prime | Operation::Next | Operation::Leave) {
                samples
            } else {
                0
            };
        let payload = input_payload
            .checked_add(if ok { block } else { 0 })
            .and_then(|n| n.checked_mul(2))
            .ok_or("payload overflow")?;
        if audit && bytes.len() - c.offset < payload {
            return Ok(report);
        }
        let input = if ok && matches!(op, Operation::Prime | Operation::Next | Operation::Leave) {
            c.samples(samples)?
        } else {
            vec![0; samples]
        };
        let expected = if ok { c.samples(block)? } else { Vec::new() };
        let mut output = vec![0x1234; block];
        let actual = if let Some(ref abi) = abi {
            let mut actual_chosen = usize::MAX;
            let code = match op {
                Operation::Prime => 0,
                Operation::Next => 1,
                Operation::Hold => 2,
                Operation::Leave => 3,
                Operation::Fade => 4,
            };
            let result = unsafe {
                tsfp_wsola_backend::tsfp_wsola_process(
                    abi.0,
                    code,
                    input.as_ptr(),
                    frames,
                    nominal,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut actual_chosen,
                )
            };
            if op == Operation::Next && result && actual_chosen != chosen {
                return Err(format!("call {} chosen offset differs", report.calls));
            }
            if !result && (output.iter().any(|&s| s != 0x1234) || actual_chosen != usize::MAX) {
                return Err(format!("call {} failure wrote output", report.calls));
            }
            result
        } else {
            let engine = engine.as_mut().expect("direct replay engine");
            match op {
                Operation::Prime => engine.prime(&input, &mut output),
                Operation::Next => match engine.next(&input, nominal, &mut output) {
                    Some(n) => {
                        if n != chosen {
                            return Err(format!("call {} chosen offset differs", report.calls));
                        }
                        true
                    }
                    None => false,
                },
                Operation::Hold => engine.hold(&mut output),
                Operation::Leave => engine.leave(&input, &mut output),
                Operation::Fade => engine.fade_out(&mut output),
            }
        };
        let actual_primed = if let Some(ref abi) = abi {
            unsafe { tsfp_wsola_backend::tsfp_wsola_primed(abi.0) }
        } else {
            engine.as_ref().expect("direct replay engine").primed()
        };
        if actual != ok || actual_primed != primed || (ok && output != expected) {
            return Err(format!("call {} differs", report.calls));
        }
        if op == Operation::Prime && ok {
            report.primes += 1;
        }
        report.calls += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_and_oversized_capture() {
        assert!(compare(&b""[..]).is_err());
        assert!(read_bounded(&[0; 65][..], 64).is_err());
    }

    #[test]
    fn native_abi_create_failure_is_not_conformance() {
        let mut invalid = b"WSOLAC1\n".to_vec();
        invalid.extend_from_slice(&0u64.to_le_bytes());
        invalid.extend_from_slice(&4u64.to_le_bytes());
        assert!(compare_native_abi(&invalid[..]).is_err());
    }
}
