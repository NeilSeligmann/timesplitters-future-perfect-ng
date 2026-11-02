//! Replay of the C host command log (TSFP_AUDIO_CMD_LOG, src/audio/dsound_audio_runtime.c).
//! Line grammar (text, hex payloads): START rate tps | SA key ticks hz mono ok hex | SP key ticks hz ok hex |
//! CUT/VOL/RVOL/RUN/FRQ/PB/PAB/STB/FRB/LPB/VLB/RST ... ok | X_* ok | R now cap frames fnv1a.

use crate::adpcm;
use crate::completion::{Completion, Event, Process};
use crate::contract::{Command, SoundHost};
use crate::log_hex::unhex;
use crate::mixer::Mixer;
use crate::report_counts::bump;

#[derive(Default, Debug)]
pub struct Report {
    pub commands: usize,
    pub refused_by_c: usize,
    /// Commands whose acceptance differs between the C host and the Rust mixer, by opcode.
    pub verdict_mismatch: Vec<(String, usize)>,
    /// Opcodes the Rust mixer does not implement yet (it answered false while C said true).
    pub unsupported: Vec<(String, usize)>,
    pub renders: usize,
    pub render_equal: usize,
    pub render_diff: usize,
    /// Renders whose PCM contains a nonzero sample and matched the C hash (silence trivially matches).
    pub nonsilent_equal: usize,
    pub frames_compared: u64,
    pub first_diff_line: Option<usize>,
    /// Completion-model observations compared (status words, deliveries, verdicts) and the mismatches by opcode.
    pub completion_checks: usize,
    pub completion_mismatch: Vec<(String, usize)>,
    pub completion_ops: Vec<(String, usize)>,
}

pub fn fnv1a(pcm: &[i16]) -> u64 {
    let mut h = 1469598103934665603u64;
    for s in pcm {
        for b in s.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(1099511628211);
        }
    }
    h
}

/// Replay `log` through a fresh [`Mixer`]; the C render lines are compared by hash.
pub fn replay(log: &str) -> Report {
    let mut rep = Report::default();
    let mut mixer: Option<Mixer> = None;
    let mut comp: Option<Completion> = None;
    let mut dowork = false;
    let mut pending: Vec<(u32, u32, u32)> = vec![];
    let mut last_runtime: Option<bool> = None;
    for (n, line) in log.lines().enumerate() {
        let f: Vec<&str> = line.split(' ').collect();
        let op = f[0];
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let c_ok = |i: usize| num(i) != 0;
        let cmd = match op {
            "START" => {
                mixer = Some(Mixer::new(num(1) as u32, num(2)));
                let mut c = Completion::new(num(2));
                c.dowork_delivery = dowork;
                comp = Some(c);
                continue;
            }
            "CCFG" => {
                dowork = num(1) != 0;
                if let Some(c) = comp.as_mut() {
                    c.dowork_delivery = dowork;
                }
                continue;
            }
            "CSD" => {
                pending.push((num(1) as u32, num(2) as u32, num(3) as u32));
                continue;
            }
            op if matches!(
                op,
                "CPN"
                    | "CSS"
                    | "CSP"
                    | "CSF"
                    | "CSM"
                    | "CW"
                    | "CBP"
                    | "CBS"
                    | "CBA"
                    | "CBF"
                    | "CBL"
                    | "CBG"
            ) =>
            {
                let Some(c) = comp.as_mut() else { continue };
                bump(&mut rep.completion_ops, op);
                let rt = last_runtime.take();
                let mut runtime = || rt.unwrap_or(false);
                let check = |ok: bool, rep: &mut Report| {
                    rep.completion_checks += 1;
                    if !ok {
                        bump(&mut rep.completion_mismatch, op);
                    }
                };
                let events: Vec<Event> = match op {
                    "CPN" => c
                        .pause_note(num(1) as u32, num(2) as u32, num(3))
                        .unwrap_or_default(),
                    "CSS" => {
                        let (v, ev) = c
                            .stream_status(num(1) as u32, num(2), num(3), num(4) as u32)
                            .unwrap_or((u32::MAX, vec![]));
                        check(v == num(5) as u32, &mut rep);
                        ev
                    }
                    "CSP" => {
                        let (r, ev) = c
                            .stream_process(
                                num(1) as u32,
                                num(2),
                                num(3),
                                num(4) as u32,
                                num(5) as u32,
                                num(6) as u32,
                                num(7) as u32,
                                num(9) as u32,
                                num(8) as u32,
                            )
                            .map(|(r, e)| (Some(r), e))
                            .unwrap_or((None, vec![]));
                        check(
                            (r == Some(Process::ListFull)) == (num(10) != 0) && r.is_some(),
                            &mut rep,
                        );
                        ev
                    }
                    "CSF" => {
                        let (ok, ev) = c
                            .stream_frequency(
                                num(1) as u32,
                                num(2),
                                num(3),
                                num(4) as u32,
                                num(5) as u32,
                                num(6) != 0,
                                &mut runtime,
                            )
                            .unwrap_or((false, vec![]));
                        check(ok == (num(7) != 0), &mut rep);
                        ev
                    }
                    "CSM" => {
                        let (ok, ev) = c
                            .stream_format(
                                num(1) as u32,
                                num(2),
                                num(3),
                                num(4) as u32,
                                num(5) as u32,
                                true,
                                &mut runtime,
                            )
                            .unwrap_or((false, vec![]));
                        check(ok == (num(6) != 0), &mut rep);
                        ev
                    }
                    "CW" => c.work(num(1)).unwrap_or_default(),
                    "CBP" => {
                        c.buffer_play(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4) != 0,
                            num(5),
                            num(7) as u32,
                            num(8),
                            num(9),
                        );
                        vec![]
                    }
                    "CBS" => {
                        c.buffer_stop(num(1) as u32, num(2), num(3), &mut || true);
                        vec![]
                    }
                    "CBA" => {
                        let ok = c.buffer_pause(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4) as u32,
                            &mut |_| true,
                        );
                        check(ok == (num(5) != 0), &mut rep);
                        vec![]
                    }
                    "CBF" => {
                        c.buffer_frequency(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4) as u32,
                            num(5) as u32,
                            &mut || true,
                        );
                        vec![]
                    }
                    "CBL" => {
                        c.buffer_loop(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4),
                            num(5),
                            num(6) as u32,
                            &mut || true,
                        );
                        vec![]
                    }
                    "CBG" => {
                        let v = c.buffer_status(num(1) as u32, num(2), num(3));
                        check(v == num(4) as u32, &mut rep);
                        vec![]
                    }
                    _ => {
                        bump(&mut rep.unsupported, op);
                        continue;
                    }
                };
                // deliveries (status 0) must equal the CSD lines the C model logged during this call
                let mut got: Vec<(u32, u32, u32)> = events
                    .iter()
                    .filter(|e| e.status == 0)
                    .map(|e| (e.stream, e.completed, e.size))
                    .collect();
                let mut want = std::mem::take(&mut pending);
                got.sort_unstable();
                want.sort_unstable();
                check(got == want, &mut rep);
                continue;
            }
            "R" => {
                let Some(m) = mixer.as_mut() else { continue };
                rep.renders += 1;
                let out = m.render_until(num(1), num(2) as usize);
                let want = u64::from_str_radix(f[4], 16).unwrap_or(0);
                if out.len() / 2 == num(3) as usize && fnv1a(&out) == want {
                    rep.render_equal += 1;
                    rep.frames_compared += (out.len() / 2) as u64;
                    if out.iter().any(|&x| x != 0) {
                        rep.nonsilent_equal += 1;
                    }
                } else {
                    rep.render_diff += 1;
                    rep.first_diff_line.get_or_insert(n + 1);
                }
                continue;
            }
            "SA" => Command::SubmitAdpcm {
                key: num(1) as u32,
                ticks: num(2),
                source_hz: num(3) as u32,
                mono: num(4) != 0,
                data: unhex(f[6]),
            },
            "SP" => {
                let b = unhex(f[5]);
                Command::SubmitPcm16Stereo {
                    key: num(1) as u32,
                    ticks: num(2),
                    source_hz: num(3) as u32,
                    pcm: b
                        .chunks(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]))
                        .collect(),
                }
            }
            "CUT" => Command::CutStream {
                key: num(1) as u32,
                ticks: num(2),
            },
            "VOL" => Command::SetStreamVolume {
                key: num(1) as u32,
                millibels: f[2].parse().unwrap_or(0),
            },
            "RVOL" => Command::ResetStreamVolume { key: num(1) as u32 },
            "RUN" => Command::SetStreamRunning {
                key: num(1) as u32,
                ticks: num(2),
                running: num(3) != 0,
            },
            "FRQ" => Command::SetStreamFrequency {
                key: num(1) as u32,
                ticks: num(2),
                source_hz: num(3) as u32,
            },
            "RST" => Command::ResetBuffers,
            "RT" => {
                let mut bins = [0u8; 8];
                let mut bin_volume = [0u32; 32];
                for k in 0..8 {
                    bins[k] = num(9 + k) as u8;
                }
                for k in 0..32 {
                    bin_volume[k] = num(17 + k) as u32;
                }
                Command::RouteStream {
                    key: num(1) as u32,
                    serial: num(2),
                    ticks: num(3),
                    routing: crate::contract::Routing {
                        volume: num(4) as u32,
                        headroom: num(5) as u32,
                        aggregate: num(6) as u32,
                        count: num(7) as u32,
                        bins,
                        bin_volume,
                    },
                }
            }
            "FS" => Command::FormatStream {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                old_hz: num(4) as u32,
                new_hz: num(5) as u32,
            },
            "FQ" => Command::FrequencyStream {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                base_hz: num(4) as u32,
                old_hz: num(5) as u32,
                new_hz: num(6) as u32,
            },
            "HE" => Command::EnableHeadroom {
                enabled: num(1) != 0,
            },
            "HB" => Command::BindHeadroom { identity: num(1) },
            "HS" => Command::SetHeadroom {
                identity: num(1),
                ticks: num(2),
                bin: num(3) as u32,
                headroom: num(4) as u32,
            },
            "PB" => {
                // runtime gate: 36 byte blocks, at most 36*32768 bytes, valid headers
                let data = unhex(f[11]);
                let pcm = if data.is_empty() || data.len() > 36 * 32768 || data.len() % 36 != 0 {
                    None
                } else {
                    adpcm::decode(&data, 1)
                };
                let want = c_ok(10);
                rep.commands += 1;
                if !want {
                    rep.refused_by_c += 1;
                }
                let got = match (pcm, mixer.as_mut()) {
                    (Some(pcm), Some(m)) => m.apply(&Command::PlayBuffer {
                        key: num(1) as u32,
                        serial: num(2),
                        ticks: num(3),
                        frequency: num(4) as u32,
                        pcm,
                        paused: num(5) != 0,
                        looping: num(6) != 0,
                        loop_start: num(7) as usize,
                        loop_end: num(8) as usize,
                        volume: f[9].parse().unwrap_or(0),
                    }),
                    _ => false,
                };
                if got != want {
                    bump(&mut rep.verdict_mismatch, op);
                }
                continue;
            }
            "PAB" => Command::PauseBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                paused: num(4) != 0,
            },
            "STB" => Command::StopBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
            },
            "FRB" => Command::FrequencyBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                frequency: num(4) as u32,
            },
            "VLB" => Command::VolumeBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                volume: f[4].parse().unwrap_or(0),
            },
            "LPB" => Command::LoopBuffer {
                key: num(1) as u32,
                serial: num(2),
                ticks: num(3),
                loop_start: num(4) as usize,
                loop_end: num(5) as usize,
            },
            _ => {
                rep.commands += 1;
                bump(&mut rep.unsupported, op);
                continue;
            }
        };
        let ok_index = match op {
            "SA" => 5,
            "SP" => 4,
            "CUT" => 3,
            "VOL" => 3,
            "RVOL" => 2,
            "RUN" | "FRQ" => 4,
            "RT" => 8,
            "FS" => 6,
            "FQ" => 7,
            "HE" => 2,
            "HB" => 2,
            "HS" => 5,
            "PAB" => 5,
            "STB" => 4,
            "FRB" => 5,
            "VLB" => 5,
            "LPB" => 6,
            _ => 99,
        };
        rep.commands += 1;
        let want = if ok_index == 99 { true } else { c_ok(ok_index) };
        if !want {
            rep.refused_by_c += 1;
        }
        if let Some(m) = mixer.as_mut() {
            let got = m.apply(&cmd);
            if matches!(op, "FQ" | "FS") {
                last_runtime = Some(got);
            }
            if got != want {
                bump(&mut rep.verdict_mismatch, op);
            }
        }
    }
    rep
}
