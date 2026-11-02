//! Strict grammar of src/audio/dsound_audio_runtime.c CMDLOG records.
use tsfp_audio::contract::{Command, Routing};

#[derive(Debug)]
pub(crate) enum Record {
    Envelope,
    End(u64, u64),
    Start(u32, u64),
    Apply(Command, bool),
    RefusedBuffer,
    Render {
        ticks: u64,
        capacity: usize,
        frames: usize,
        hash: u64,
    },
    Completion,
}

struct Fields<'a>(Vec<&'a str>);
impl Fields<'_> {
    fn count(&self, n: usize) -> Result<(), String> {
        if self.0.len() == n {
            Ok(())
        } else {
            Err("wrong field count".into())
        }
    }
    fn number<T: std::str::FromStr>(&self, n: usize) -> Result<T, String> {
        self.0
            .get(n)
            .ok_or("missing field")?
            .parse()
            .map_err(|_| "invalid or overflowing number".into())
    }
    fn flag(&self, n: usize) -> Result<bool, String> {
        match self.number::<u8>(n)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid boolean".into()),
        }
    }
    fn bytes(&self, n: usize) -> Result<Vec<u8>, String> {
        let hex = self.0[n].as_bytes();
        if hex.len() % 2 != 0 {
            return Err("odd hex payload length".into());
        }
        fn digit(c: u8) -> Result<u8, String> {
            match c {
                b'0'..=b'9' => Ok(c - b'0'),
                b'a'..=b'f' => Ok(c - b'a' + 10),
                b'A'..=b'F' => Ok(c - b'A' + 10),
                _ => Err("invalid hex payload".into()),
            }
        }
        hex.chunks_exact(2)
            .map(|p| Ok(digit(p[0])? * 16 + digit(p[1])?))
            .collect()
    }
}

pub(crate) fn parse(line: &str) -> Result<Record, String> {
    // The producer uses one space and can emit an empty final payload.
    let f = Fields(line.split(' ').take(50).collect());
    let op = f.0[0];
    let n = match op {
        "CAPTURE" => 2,
        "END" => 3,
        "START" => 3,
        "SA" => 7,
        "SP" => 6,
        "CUT" | "VOL" => 4,
        "RVOL" | "HE" | "HB" => 3,
        "RUN" | "FRQ" => 5,
        "RST" => 1,
        "RT" => 49,
        "FS" => 7,
        "FQ" => 8,
        "HS" => 6,
        "PB" => 12,
        "PAB" | "FRB" | "VLB" => 6,
        "STB" => 5,
        "LPB" => 7,
        "R" => 5,
        // Completion observations do not mutate the mixer. Validate their shape,
        // count them explicitly, and make no completion-conformance claim.
        "CCFG" | "CW" => 2,
        "CPN" | "CBS" => 4,
        "CSD" | "CBG" => 5,
        "CSS" | "CBA" | "CBF" => 6,
        "CSM" | "CBL" => 7,
        "CSF" => 8,
        "CSP" | "CBP" => 11,
        _ => return Err("unknown opcode".into()),
    };
    f.count(n)?;
    if op == "CAPTURE" {
        if f.0[1] != "1" {
            return Err("unsupported capture version".into());
        }
        return Ok(Record::Envelope);
    }
    if op == "END" {
        if f.0[2].len() != 16 {
            return Err("invalid capture seal length".into());
        }
        let hash = u64::from_str_radix(f.0[2], 16).map_err(|_| "invalid capture seal")?;
        return Ok(Record::End(f.number(1)?, hash));
    }
    if matches!(
        op,
        "CCFG"
            | "CW"
            | "CPN"
            | "CBS"
            | "CSD"
            | "CBG"
            | "CSS"
            | "CBA"
            | "CBF"
            | "CSM"
            | "CBL"
            | "CSF"
            | "CSP"
            | "CBP"
    ) {
        for i in 1..n {
            f.number::<u64>(i)?;
        }
        return Ok(Record::Completion);
    }
    if op == "START" {
        let rate = f.number::<u32>(1)?;
        let tps = f.number::<u64>(2)?;
        if rate == 0 || rate > 384_000 || tps == 0 {
            return Err("invalid clock or output rate".into());
        }
        return Ok(Record::Start(rate, tps));
    }
    if op == "R" {
        let capacity = f.number(2)?;
        let frames = f.number(3)?;
        if capacity > 1_048_576 || frames > capacity {
            return Err("render exceeds bound".into());
        }
        if f.0[4].len() != 16 {
            return Err("invalid render hash length".into());
        }
        let hash = u64::from_str_radix(f.0[4], 16).map_err(|_| "invalid render hash")?;
        return Ok(Record::Render {
            ticks: f.number(1)?,
            capacity,
            frames,
            hash,
        });
    }
    let (command, ok) = match op {
        "SA" => (
            Command::SubmitAdpcm {
                key: f.number(1)?,
                ticks: f.number(2)?,
                source_hz: f.number(3)?,
                mono: f.flag(4)?,
                data: f.bytes(6)?,
            },
            f.flag(5)?,
        ),
        "SP" => {
            let data = f.bytes(5)?;
            if data.len() % 4 != 0 {
                return Err("PCM payload is not stereo i16".into());
            }
            (
                Command::SubmitPcm16Stereo {
                    key: f.number(1)?,
                    ticks: f.number(2)?,
                    source_hz: f.number(3)?,
                    pcm: data
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]]))
                        .collect(),
                },
                f.flag(4)?,
            )
        }
        "CUT" => (
            Command::CutStream {
                key: f.number(1)?,
                ticks: f.number(2)?,
            },
            f.flag(3)?,
        ),
        "VOL" => (
            Command::SetStreamVolume {
                key: f.number(1)?,
                millibels: f.number(2)?,
            },
            f.flag(3)?,
        ),
        "RVOL" => (Command::ResetStreamVolume { key: f.number(1)? }, f.flag(2)?),
        "RUN" => (
            Command::SetStreamRunning {
                key: f.number(1)?,
                ticks: f.number(2)?,
                running: f.flag(3)?,
            },
            f.flag(4)?,
        ),
        "FRQ" => (
            Command::SetStreamFrequency {
                key: f.number(1)?,
                ticks: f.number(2)?,
                source_hz: f.number(3)?,
            },
            f.flag(4)?,
        ),
        "RST" => (Command::ResetBuffers, true),
        "RT" => {
            let mut bins = [0; 8];
            let mut bin_volume = [0; 32];
            for (i, v) in bins.iter_mut().enumerate() {
                *v = f.number(9 + i)?;
            }
            for (i, v) in bin_volume.iter_mut().enumerate() {
                *v = f.number(17 + i)?;
            }
            (
                Command::RouteStream {
                    key: f.number(1)?,
                    serial: f.number(2)?,
                    ticks: f.number(3)?,
                    routing: Routing {
                        volume: f.number(4)?,
                        headroom: f.number(5)?,
                        aggregate: f.number(6)?,
                        count: f.number(7)?,
                        bins,
                        bin_volume,
                    },
                },
                f.flag(8)?,
            )
        }
        "FS" => (
            Command::FormatStream {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                old_hz: f.number(4)?,
                new_hz: f.number(5)?,
            },
            f.flag(6)?,
        ),
        "FQ" => (
            Command::FrequencyStream {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                base_hz: f.number(4)?,
                old_hz: f.number(5)?,
                new_hz: f.number(6)?,
            },
            f.flag(7)?,
        ),
        "HE" => (
            Command::EnableHeadroom {
                enabled: f.flag(1)?,
            },
            f.flag(2)?,
        ),
        "HB" => (
            Command::BindHeadroom {
                identity: f.number(1)?,
            },
            f.flag(2)?,
        ),
        "HS" => (
            Command::SetHeadroom {
                identity: f.number(1)?,
                ticks: f.number(2)?,
                bin: f.number(3)?,
                headroom: f.number(4)?,
            },
            f.flag(5)?,
        ),
        "PB" => {
            let data = f.bytes(11)?;
            let c = crate::oracle::decode_mono(&data);
            let rust = if data.is_empty() || data.len() > 36 * 32768 || data.len() % 36 != 0 {
                None
            } else {
                tsfp_audio::adpcm::decode(&data, 1)
            };
            if c != rust {
                return Err("PB C/Rust decoder mismatch".into());
            }
            let command = Command::PlayBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                frequency: f.number(4)?,
                paused: f.flag(5)?,
                looping: f.flag(6)?,
                loop_start: f.number(7)?,
                loop_end: f.number(8)?,
                volume: f.number(9)?,
                pcm: rust.unwrap_or_default(),
            };
            let accepted = f.flag(10)?;
            if c.is_none() {
                if accepted {
                    return Err("recorded acceptance of invalid PB".into());
                }
                return Ok(Record::RefusedBuffer);
            }
            (command, accepted)
        }
        "PAB" => (
            Command::PauseBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                paused: f.flag(4)?,
            },
            f.flag(5)?,
        ),
        "STB" => (
            Command::StopBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
            },
            f.flag(4)?,
        ),
        "FRB" => (
            Command::FrequencyBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                frequency: f.number(4)?,
            },
            f.flag(5)?,
        ),
        "VLB" => (
            Command::VolumeBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                volume: f.number(4)?,
            },
            f.flag(5)?,
        ),
        "LPB" => (
            Command::LoopBuffer {
                key: f.number(1)?,
                serial: f.number(2)?,
                ticks: f.number(3)?,
                loop_start: f.number(4)?,
                loop_end: f.number(5)?,
            },
            f.flag(6)?,
        ),
        _ => unreachable!(),
    };
    Ok(Record::Apply(command, ok))
}

#[cfg(test)]
#[path = "capture_parse_tests.rs"]
mod tests;
