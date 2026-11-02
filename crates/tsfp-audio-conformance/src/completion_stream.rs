//! Strict bounded stream completion capture parsing. No raw guest data in errors.
use crate::completion_capture::{self, Delivery, Observation, Operation};
use std::io::Read;

const LIMIT: usize = 64 * 1024 * 1024;
const INIT: u64 = 1469598103934665603;
fn number(s: &str) -> Result<u64, String> {
    s.parse().map_err(|_| "invalid number".into())
}
fn word(s: &str) -> Result<u32, String> {
    u32::try_from(number(s)?).map_err(|_| "word overflow".into())
}
fn write_result(s: &str, address: u32) -> Result<Option<bool>, String> {
    match (number(s)?, address) {
        (0, 0) => Ok(None),
        (1, a) if a != 0 => Ok(Some(true)),
        (2, a) if a != 0 => Ok(Some(false)),
        _ => Err("invalid write outcome".into()),
    }
}

/// Compare a complete, sealed reference stream. A failed publication is incomplete.
pub fn compare(source: impl Read) -> Result<usize, String> {
    compare_version(source, false)
}

/// Version 2 explains only accepted format transitions with no pending abort writes.
pub fn compare_v2(source: impl Read) -> Result<usize, String> {
    compare_version(source, true)
}

fn compare_version(mut source: impl Read, explain_empty_format: bool) -> Result<usize, String> {
    let mut bytes = Vec::new();
    source
        .by_ref()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "read failure")?;
    if bytes.len() > LIMIT || !bytes.ends_with(b"\n") {
        return Err("truncated or oversized capture".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "invalid UTF-8")?;
    let mut hash = INIT;
    let mut lines = 0u64;
    let mut clock = None;
    let mut mode = false;
    let mut pending = Vec::new();
    let mut process: Option<(u32, u64, u64, u32)> = None;
    let mut expected_format_gap = false;
    let mut observations = Vec::new();
    let mut sealed = false;
    for line in text.split_inclusive('\n') {
        if sealed || line.len() > 256 {
            return Err("trailing or oversized record".into());
        }
        let fields: Vec<_> = line.trim_end_matches('\n').split(' ').collect();
        let op = fields.first().copied().unwrap_or("");
        if op == "END" {
            if fields.len() != 3
                || number(fields[1])? != lines
                || fields[2].len() != 16
                || u64::from_str_radix(fields[2], 16).ok() != Some(hash)
                || !pending.is_empty()
                || process.is_some()
                || expected_format_gap
            {
                return Err("invalid seal or orphan delivery".into());
            }
            sealed = true;
            continue;
        }
        for b in line.bytes() {
            hash = (hash ^ u64::from(b)).wrapping_mul(1099511628211);
        }
        lines += 1;
        if lines > 1_000_000 {
            return Err("too many records".into());
        }
        if lines == 1 {
            if line != "ACMP1\n" {
                return Err("bad header".into());
            }
            continue;
        }
        if lines == 2 {
            if fields.len() != 3 || op != "CFG" {
                return Err("missing initial configuration".into());
            }
            let hz = number(fields[1])?;
            if hz == 0 {
                return Err("invalid clock".into());
            }
            mode = match fields[2] {
                "0" => false,
                "1" => true,
                _ => return Err("invalid mode".into()),
            };
            clock = Some(hz);
            continue;
        }
        if expected_format_gap {
            if line != "GAP unsupported-format-or-frequency\n" {
                return Err("missing format gap marker".into());
            }
            expected_format_gap = false;
            continue;
        }
        if process.is_some() && op != "CSP" {
            return Err("interleaved or incomplete process".into());
        }
        let operation = match op {
            "CSD" if fields.len() == 7 => {
                let completed = word(fields[2])?;
                let status_address = word(fields[4])?;
                pending.push(Delivery {
                    stream: word(fields[1])?,
                    completed,
                    size: word(fields[3])?,
                    status_address,
                    completed_ok: write_result(fields[5], completed)?,
                    status_ok: write_result(fields[6], status_address)?,
                });
                continue;
            }
            "CSM" if explain_empty_format && fields.len() == 8 && fields[7] == "1" => {
                if !pending.is_empty() {
                    return Err("unmatched delivery before format".into());
                }
                let _format_address = word(fields[6])?;
                expected_format_gap = true;
                Operation::Format {
                    stream: word(fields[1])?,
                    serial: number(fields[2])?,
                    ticks: number(fields[3])?,
                    old_hz: word(fields[4])?,
                    new_hz: word(fields[5])?,
                }
            }
            "CPB" if fields.len() == 6 => {
                let stream = word(fields[1])?;
                let serial = number(fields[2])?;
                let ticks = number(fields[3])?;
                let max_packets = word(fields[4])?;
                let full = match fields[5] {
                    "0" => false,
                    "1" => true,
                    _ => return Err("invalid begin verdict".into()),
                };
                if !full {
                    process = Some((stream, serial, ticks, max_packets));
                }
                Operation::Begin {
                    stream,
                    serial,
                    ticks,
                    max_packets,
                    full,
                }
            }
            "CSP" if fields.len() == 11 && fields[10] == "0" => {
                let stream = word(fields[1])?;
                let serial = number(fields[2])?;
                let ticks = number(fields[3])?;
                let max_packets = word(fields[8])?;
                if process.take() != Some((stream, serial, ticks, max_packets)) {
                    return Err("unmatched process".into());
                }
                Operation::Push {
                    stream,
                    serial,
                    ticks,
                    size: word(fields[4])?,
                    completed: word(fields[5])?,
                    status_address: word(fields[6])?,
                    average: word(fields[7])?,
                    frequency: word(fields[9])?,
                }
            }
            "CSS" if fields.len() == 6 => Operation::Status {
                stream: word(fields[1])?,
                serial: number(fields[2])?,
                ticks: number(fields[3])?,
                max_packets: word(fields[4])?,
                value: word(fields[5])?,
            },
            "CPN" if fields.len() == 4 => Operation::Pause {
                stream: word(fields[1])?,
                mode: word(fields[2])?,
                ticks: number(fields[3])?,
            },
            "CW" if fields.len() == 2 => Operation::Work {
                ticks: number(fields[1])?,
            },
            _ => return Err("unsupported or malformed operation".into()),
        };
        observations.push(Observation {
            operation,
            deliveries: std::mem::take(&mut pending),
        });
    }
    if !sealed {
        return Err("missing seal".into());
    }
    completion_capture::compare_observations(
        clock.ok_or("missing configuration")?,
        mode,
        &observations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seal(body: &str) -> String {
        let hash = body
            .bytes()
            .fold(INIT, |h, b| (h ^ u64::from(b)).wrapping_mul(1099511628211));
        format!("{body}END {} {hash:016x}\n", body.lines().count())
    }
    #[test]
    fn rejects_gaps_and_ordered_write_mutants() {
        let body = "ACMP1\nCFG 1000 0\nCPB 7 1 0 4 0\nCSP 7 1 0 100 20 24 100 4 0 0\nCPN 7 0 0\nCSD 7 20 100 24 1 1\nCSS 7 1 1000 4 1\n";
        assert_eq!(compare(seal(body).as_bytes()), Ok(1));
        assert!(compare(body.as_bytes()).is_err());
        assert!(compare(seal(&body.replace("24 1 1", "25 1 1")).as_bytes()).is_err());
        assert!(compare(seal(&body.replace("24 1 1", "24 1 2")).as_bytes()).is_err());
        assert!(
            compare(
                seal(&body.replace(
                    "CPN 7 0 0",
                    "CSF 7 1 0 44100 32000 0 1\nGAP unsupported-format-or-frequency"
                ))
                .as_bytes()
            )
            .is_err()
        );
        assert!(
            compare(
                seal(&body.replace(
                    "CPN 7 0 0",
                    "CSM 7 1 0 44100 32000 123 1\nGAP unsupported-format-or-frequency"
                ))
                .as_bytes()
            )
            .is_err()
        );
        assert!(compare(seal(&body.replace("CSP 7 1 0", "CSP 8 1 0")).as_bytes()).is_err());
        assert!(compare(seal(&body.replace("CSP 7 1 0", "CW 0\nCSP 7 1 0")).as_bytes()).is_err());
        assert!(
            compare(seal(&body.replace("CPN 7 0 0", "GAP configuration-change")).as_bytes())
                .is_err()
        );
        assert!(
            compare(seal(&body.replace("CSP 7 1 0 100 20 24 100 4 0 0\n", "")).as_bytes()).is_err()
        );
        assert!(compare(seal(&(body.to_owned() + "CSD 7 20 100 24 1 1\n")).as_bytes()).is_err());
        assert!(compare(seal("ACMP1\nCFG 1000 0\n").as_bytes()).is_err());
    }

    #[test]
    fn v2_explains_only_accepted_empty_format_transition() {
        let body = "ACMP1\nCFG 1000 0\nCPN 7 1 0\nCSM 7 1 0 44100 32000 123 1\nGAP unsupported-format-or-frequency\nCPB 7 1 0 4 0\nCSP 7 1 0 100 20 24 100 4 44100 0\nCPN 7 0 0\nCSD 7 20 100 24 1 1\nCSS 7 1 2000 4 1\n";
        assert!(compare(seal(body).as_bytes()).is_err());
        assert_eq!(compare_v2(seal(body).as_bytes()), Ok(1));
        assert!(compare_v2(seal(&body.replace("32000 123 1", "32000 123 0")).as_bytes()).is_err());
        assert!(
            compare_v2(seal(&body.replace("GAP unsupported-format-or-frequency\n", "")).as_bytes())
                .is_err()
        );
        let queued=body.replace("CPN 7 1 0\nCSM 7 1 0 44100 32000 123 1\nGAP unsupported-format-or-frequency\n", "").replace("CSP 7 1 0 100 20 24 100 4 44100 0\n", "CSP 7 1 0 100 20 24 100 4 44100 0\nCSM 7 1 0 44100 32000 123 1\nGAP unsupported-format-or-frequency\n");
        assert!(compare_v2(seal(&queued).as_bytes()).is_err());
    }
}
