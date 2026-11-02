//! Incremental parser of one `TSFP_AUDIO_CMD_LOG` mixer line (T2136 slice 2). The same grammar and field
//! positions as [`crate::replay::replay`], split out so a live thread can feed a [`crate::mixer::Mixer`] line by line.
//! Completion-model lines (`C*`, `CSD`, `CCFG`) belong to the guest adapter and are reported as [`Line::Completion`].

use crate::adpcm;
use crate::contract::{Command, Routing};
use crate::log_hex::unhex;

#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Start {
        rate: u32,
        ticks_per_second: u64,
    },
    /// A mixer command. `command` is `None` when the runtime gate refused it before the mixer (bad `PB` ADPCM). `c_ok` is the
    /// verdict the C host logged (`None` for opcodes without one).
    Command {
        op: String,
        command: Option<Command>,
        c_ok: Option<bool>,
    },
    /// `R now capacity frames fnv1a`
    Render {
        now: u64,
        capacity: usize,
        frames: usize,
        hash: u64,
    },
    /// `CCFG CPN CSS ... CBG CSD`: not a mixer concern.
    Completion,
    /// Blank, comment or unknown opcode.
    Other,
}

pub fn parse_line(line: &str) -> Line {
    let f: Vec<&str> = line.split(' ').collect();
    let op = f[0];
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let text = |i: usize| f.get(i).copied().unwrap_or("");
    let int = |i: usize| text(i).parse::<i32>().unwrap_or(0);
    match op {
        "START" => {
            return Line::Start {
                rate: num(1) as u32,
                ticks_per_second: num(2),
            };
        }
        "CCFG" | "CSD" | "CPN" | "CSS" | "CSP" | "CSF" | "CSM" | "CW" | "CBP" | "CBS" | "CBA"
        | "CBF" | "CBL" | "CBG" => return Line::Completion,
        "R" => {
            return Line::Render {
                now: num(1),
                capacity: num(2) as usize,
                frames: num(3) as usize,
                hash: u64::from_str_radix(text(4), 16).unwrap_or(0),
            };
        }
        _ => {}
    }
    let (command, ok_index): (Option<Command>, Option<usize>) = match op {
        "SA" => (
            Some(Command::SubmitAdpcm {
                key: num(1) as u32,
                ticks: num(2),
                source_hz: num(3) as u32,
                mono: num(4) != 0,
                data: unhex(text(6)),
            }),
            Some(5),
        ),
        "SP" => {
            let b = unhex(text(5));
            let pcm = b
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            (
                Some(Command::SubmitPcm16Stereo {
                    key: num(1) as u32,
                    ticks: num(2),
                    source_hz: num(3) as u32,
                    pcm,
                }),
                Some(4),
            )
        }
        "CUT" => (
            Some(Command::CutStream {
                key: num(1) as u32,
                ticks: num(2),
            }),
            Some(3),
        ),
        "VOL" => (
            Some(Command::SetStreamVolume {
                key: num(1) as u32,
                millibels: int(2),
            }),
            Some(3),
        ),
        "RVOL" => (
            Some(Command::ResetStreamVolume { key: num(1) as u32 }),
            Some(2),
        ),
        "RUN" => (
            Some(Command::SetStreamRunning {
                key: num(1) as u32,
                ticks: num(2),
                running: num(3) != 0,
            }),
            Some(4),
        ),
        "FRQ" => (
            Some(Command::SetStreamFrequency {
                key: num(1) as u32,
                ticks: num(2),
                source_hz: num(3) as u32,
            }),
            Some(4),
        ),
        "RST" => (Some(Command::ResetBuffers), None),
        "RT" => {
            let mut bins = [0u8; 8];
            let mut bin_volume = [0u32; 32];
            for (k, bin) in bins.iter_mut().enumerate() {
                *bin = num(9 + k) as u8;
            }
            for (k, volume) in bin_volume.iter_mut().enumerate() {
                *volume = num(17 + k) as u32;
            }
            let routing = Routing {
                volume: num(4) as u32,
                headroom: num(5) as u32,
                aggregate: num(6) as u32,
                count: num(7) as u32,
                bins,
                bin_volume,
            };
            (
                Some(Command::RouteStream {
                    key: num(1) as u32,
                    serial: num(2),
                    ticks: num(3),
                    routing,
                }),
                Some(8),
            )
        }
        "FS" => (
            Some(Command::FormatStream {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                old_hz: num(4) as u32,
                new_hz: num(5) as u32,
            }),
            Some(6),
        ),
        "FQ" => (
            Some(Command::FrequencyStream {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                base_hz: num(4) as u32,
                old_hz: num(5) as u32,
                new_hz: num(6) as u32,
            }),
            Some(7),
        ),
        "HE" => (
            Some(Command::EnableHeadroom {
                enabled: num(1) != 0,
            }),
            Some(2),
        ),
        "HB" => (Some(Command::BindHeadroom { identity: num(1) }), Some(2)),
        "HS" => (
            Some(Command::SetHeadroom {
                identity: num(1),
                ticks: num(2),
                bin: num(3) as u32,
                headroom: num(4) as u32,
            }),
            Some(5),
        ),
        "PB" => {
            // runtime gate: 36 byte blocks, at most 36*32768 bytes, valid headers
            let data = unhex(text(11));
            let pcm = if data.is_empty() || data.len() > 36 * 32768 || data.len() % 36 != 0 {
                None
            } else {
                adpcm::decode(&data, 1)
            };
            let command = pcm.map(|pcm| Command::PlayBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                frequency: num(4) as u32,
                pcm,
                paused: num(5) != 0,
                looping: num(6) != 0,
                loop_start: num(7) as usize,
                loop_end: num(8) as usize,
                volume: int(9),
            });
            (command, Some(10))
        }
        "PAB" => (
            Some(Command::PauseBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                paused: num(4) != 0,
            }),
            Some(5),
        ),
        "STB" => (
            Some(Command::StopBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
            }),
            Some(4),
        ),
        "FRB" => (
            Some(Command::FrequencyBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                frequency: num(4) as u32,
            }),
            Some(5),
        ),
        "VLB" => (
            Some(Command::VolumeBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                volume: int(4),
            }),
            Some(5),
        ),
        "LPB" => (
            Some(Command::LoopBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                loop_start: num(4) as usize,
                loop_end: num(5) as usize,
            }),
            Some(6),
        ),
        _ => return Line::Other,
    };
    Line::Command {
        op: op.to_string(),
        command,
        c_ok: ok_index.map(|i| num(i) != 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lines() {
        assert_eq!(
            parse_line("START 48000 733333333"),
            Line::Start {
                rate: 48000,
                ticks_per_second: 733333333
            }
        );
        assert_eq!(
            parse_line("R 100 512 512 00ff"),
            Line::Render {
                now: 100,
                capacity: 512,
                frames: 512,
                hash: 0xff
            }
        );
        assert_eq!(parse_line("CSD 1 2 3"), Line::Completion);
        assert_eq!(parse_line("# comment"), Line::Other);
        assert_eq!(
            parse_line("CUT 7 90 1"),
            Line::Command {
                op: "CUT".into(),
                command: Some(Command::CutStream { key: 7, ticks: 90 }),
                c_ok: Some(true)
            }
        );
        // PB with an empty payload is refused by the runtime gate before the mixer
        match parse_line("PB 1 2 3 4 0 0 0 0 0 0 ") {
            Line::Command {
                command: None,
                c_ok: Some(false),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }
}
