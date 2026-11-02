//! Adapter-mode replay: drive [`DsAdapter`] with the guest-level calls reconstructed from a C host log
//! (`TSFP_AUDIO_CMD_LOG`), over sparse fake guest memory, and compare what the adapter does with what the
//! C runtime logged: the mixer commands it emits (RUN SA PB STB PAB FRB LPB), the status words and results,
//! the completion words it writes, and the rendered PCM hashes. Reconstructed inputs are exact for the
//! logged fields (serial, tick, sizes, rates, data) and synthetic for what the C log does not carry
//! (guest addresses of the packet and data, the effective WAVEFORMATEX beyond rate/block/average).

use crate::adapter::{BufferView, DsAdapter, GuestMemory, StreamView};
use crate::adpcm;
use crate::contract::{Command, SoundHost};
use crate::log_hex::unhex;
use crate::mixer::Mixer;
use crate::recording::{Event as Rec, Recording};
use crate::replay::fnv1a;
use crate::report_counts::bump;
use std::collections::HashMap;

#[derive(Default)]
struct FakeMem {
    words: HashMap<u32, u32>,
    bytes: HashMap<u32, Vec<u8>>,
    writes: Vec<(u32, u32)>,
}
impl GuestMemory for FakeMem {
    fn read_u32(&self, a: u32) -> Option<u32> {
        Some(*self.words.get(&a).unwrap_or(&0))
    }
    fn write_u32(&mut self, a: u32, v: u32) -> bool {
        self.words.insert(a, v);
        self.writes.push((a, v));
        true
    }
    fn read_bytes(&self, a: u32, len: usize) -> Option<Vec<u8>> {
        if let Some(b) = self.bytes.get(&a) {
            return Some(b[..len.min(b.len())].to_vec());
        }
        // packet words come from `words`
        Some(
            (0..len / 4)
                .flat_map(|i| {
                    self.words
                        .get(&(a + 4 * i as u32))
                        .copied()
                        .unwrap_or(0)
                        .to_le_bytes()
                })
                .collect(),
        )
    }
    fn range_readable(&self, _: u32, _: u32) -> bool {
        true
    }
}

#[derive(Default, Debug)]
pub struct AdapterReport {
    pub calls: usize,
    pub by_op: Vec<(String, usize)>,
    pub mismatches: Vec<(String, usize)>,
    pub renders: usize,
    pub render_equal: usize,
    pub render_diff: usize,
    pub first_mismatch_line: Option<usize>,
}

fn format_bytes(mono: bool, rate: u32, average: u32) -> [u8; 20] {
    let mut f = [0u8; 20];
    f[0..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&(if mono { 1u16 } else { 2 }).to_le_bytes());
    f[4..8].copy_from_slice(&rate.to_le_bytes());
    f[8..12].copy_from_slice(&average.to_le_bytes());
    f[12..14].copy_from_slice(&(if mono { 36u16 } else { 72 }).to_le_bytes());
    f[14..16].copy_from_slice(&4u16.to_le_bytes());
    f[16..18].copy_from_slice(&2u16.to_le_bytes());
    f[18..20].copy_from_slice(&64u16.to_le_bytes());
    f
}

/// A logged runtime line of a kind the adapter produces, parsed into the Command it must equal.
fn parse_adapter_kind(op: &str, f: &[&str]) -> Option<Command> {
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    Some(match op {
        "RUN" => Command::SetStreamRunning {
            key: n(1) as u32,
            ticks: n(2),
            running: n(3) != 0,
        },
        "SA" => Command::SubmitAdpcm {
            key: n(1) as u32,
            ticks: n(2),
            source_hz: n(3) as u32,
            mono: n(4) != 0,
            data: unhex(f[6]),
        },
        "PB" => Command::PlayBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            frequency: n(4) as u32,
            pcm: adpcm::decode(&unhex(f[11]), 1)?,
            paused: n(5) != 0,
            looping: n(6) != 0,
            loop_start: n(7) as usize,
            loop_end: n(8) as usize,
            volume: f[9].parse().ok()?,
        },
        "STB" => Command::StopBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
        },
        "PAB" => Command::PauseBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            paused: n(4) != 0,
        },
        "FRB" => Command::FrequencyBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            frequency: n(4) as u32,
        },
        "LPB" => Command::LoopBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            loop_start: n(4) as usize,
            loop_end: n(5) as usize,
        },
        _ => return None,
    })
}

pub fn replay_adapter(log: &str) -> AdapterReport {
    let mut rep = AdapterReport::default();
    let mut ad: Option<DsAdapter<Recording<Mixer>>> = None;
    let mut mem = FakeMem::default();
    let mut stash: Vec<Command> = vec![];
    let mut csd: Vec<(u32, u32, u32)> = vec![];
    let mut last_runtime: Option<bool> = None;
    let mut buffers: HashMap<u32, BufferView> = HashMap::new();
    let mut sa_format: HashMap<u32, (bool, u32)> = HashMap::new();
    let mut seen = 0usize; // log entries of the adapter host already compared
    let mut deferred: std::collections::VecDeque<Command> = std::collections::VecDeque::new();
    let mut dowork = false;
    for (n, line) in log.lines().enumerate() {
        let f: Vec<&str> = line.split(' ').collect();
        let op = f[0];
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        macro_rules! miss {
            ($k:expr) => {{
                bump(&mut rep.mismatches, $k);
                rep.first_mismatch_line.get_or_insert(n + 1);
            }};
        }
        if op == "START" {
            let mut a = DsAdapter::new(num(2), Recording::new(Mixer::new(num(1) as u32, num(2))));
            a.completion.dowork_delivery = dowork;
            ad = Some(a);
            continue;
        }
        let Some(a) = ad.as_mut() else { continue };
        match op {
            "CCFG" => {
                dowork = num(1) != 0;
                a.completion.dowork_delivery = dowork;
                continue;
            }
            "CSD" => {
                csd.push((num(2) as u32, num(3) as u32, num(4) as u32));
                continue;
            }
            "R" => {
                rep.renders += 1;
                let out = a.host.render_until(num(1), num(2) as usize);
                if out.len() / 2 == num(3) as usize
                    && fnv1a(&out) == u64::from_str_radix(f[4], 16).unwrap_or(0)
                {
                    rep.render_equal += 1;
                } else {
                    rep.render_diff += 1;
                    rep.first_mismatch_line.get_or_insert(n + 1);
                }
                continue;
            }
            "RUN" | "SA" | "PB" | "STB" | "PAB" | "FRB" | "LPB" => {
                if op == "SA" {
                    sa_format.insert(num(1) as u32, (num(4) != 0, num(3) as u32));
                }
                if op == "PB" {
                    let enc = unhex(f[11]);
                    let norm = f[9].parse::<i32>().unwrap_or(0);
                    let blocks = enc.len() as u64 / 36;
                    let (ls, le) = (num(7), num(8));
                    let view = BufferView {
                        serial: num(2),
                        // Playback looping is passed to Play, not a buffer creation flag.
                        flags: 0,
                        cache_mask: 0,
                        block_align: 36,
                        frequency: num(4) as u32,
                        volume: if norm <= -10000 { -10000 } else { norm + 600 },
                        data_address: 0x30000 + (num(1) as u32 & 0xff) * 0,
                        data_length: enc.len() as u32,
                        data_sets: 1,
                        volume_sets: 1,
                        pause_sets: 1,
                        frequency_sets: 1,
                        loop_sets: u32::from(num(6) != 0 && le > 0),
                        loop_start: (ls / 64 * 36) as u32,
                        loop_length: ((le.saturating_sub(ls)) / 64 * 36) as u32,
                    };
                    let _ = blocks;
                    buffers.insert(num(1) as u32, view);
                    mem.bytes.insert(view.data_address, enc);
                }
                if let Some(c) = parse_adapter_kind(op, &f) {
                    if let Some(front) = deferred.pop_front() {
                        if front != c {
                            miss!(op);
                        }
                    } else {
                        stash.push(c);
                    }
                }
                continue;
            }
            "CPN" | "CSS" | "CSP" | "CW" | "CBP" | "CBS" | "CBG" | "CBA" | "CBF" | "CBL" => {}
            "CSF" | "CSM" => {
                csd.clear();
                let rt = last_runtime.take();
                let mut runtime = || rt.unwrap_or(false);
                let (ok, _) = if op == "CSF" {
                    a.completion
                        .stream_frequency(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4) as u32,
                            num(5) as u32,
                            num(6) != 0,
                            &mut runtime,
                        )
                        .unwrap_or((false, vec![]))
                } else {
                    a.completion
                        .stream_format(
                            num(1) as u32,
                            num(2),
                            num(3),
                            num(4) as u32,
                            num(5) as u32,
                            true,
                            &mut runtime,
                        )
                        .unwrap_or((false, vec![]))
                };
                if ok != (num(if op == "CSF" { 7 } else { 6 }) != 0) {
                    miss!(op);
                }
                continue;
            }
            _ => {
                // other runtime lines (routes, headroom, volumes, cuts, format and frequency events): applied as logged
                if let Some(cmd) = other_command(op, &f) {
                    let ok = a.host.apply(&cmd);
                    if matches!(op, "FQ" | "FS") {
                        last_runtime = Some(ok);
                    }
                }
                continue;
            }
        }
        // adapter-driven call
        rep.calls += 1;
        bump(&mut rep.by_op, op);
        mem.writes.clear();
        let before = a.host.log.len();
        let mut expect_writes: Vec<(u32, u32)> = vec![];
        match op {
            "CPN" => {
                let _ = a.stream_pause(&mut mem, num(1) as u32, num(2) as u32, num(3));
            }
            "CSS" => {
                let view = StreamView {
                    serial: num(2),
                    max_packets: num(4) as u32,
                    volume_seen: true,
                    discontinuity_seen: true,
                    format: [0; 20],
                };
                let out = 0x5000;
                let ok = a.stream_get_status(&mut mem, &view, num(1) as u32, out, num(3));
                if ok != Ok(0) || mem.words.get(&out).copied() != Some(num(5) as u32) {
                    miss!("CSS");
                }
            }
            "CSP" => {
                let stream = num(1) as u32;
                let (mono, rate) = sa_format
                    .get(&stream)
                    .copied()
                    .unwrap_or((false, num(9) as u32));
                let view = StreamView {
                    serial: num(2),
                    max_packets: num(8) as u32,
                    volume_seen: true,
                    discontinuity_seen: true,
                    format: format_bytes(mono, rate, num(7) as u32),
                };
                let (size, completed, status) = (num(4) as u32, num(5) as u32, num(6) as u32);
                let packet = 0x10000;
                for (i, w) in [0x20000, size, completed, status, 0, 0]
                    .into_iter()
                    .enumerate()
                {
                    mem.words.insert(packet + 4 * i as u32, w);
                }
                if let Some(Command::SubmitAdpcm { data, .. }) = stash
                    .iter()
                    .find(|c| matches!(c, Command::SubmitAdpcm { .. }))
                {
                    mem.bytes.insert(0x20000, data.clone());
                }
                let full = num(10) != 0;
                let r = a.stream_process(&mut mem, &view, stream, packet, 0, num(3));
                let expected = if full { Ok(0x8878_0032) } else { Ok(0) };
                if r != expected {
                    miss!("CSP");
                }
                if !full {
                    expect_writes.push((completed, 0));
                    expect_writes.push((status, 0x8000_000A));
                }
            }
            "CW" => {
                let _ = a.do_work(&mut mem, num(1));
            }
            "CBP" => {
                if let Some(view) = buffers.get(&(num(1) as u32)).copied() {
                    let flags = u32::from(num(4) != 0);
                    if a.buffer_play(&mut mem, &view, num(1) as u32, flags, num(3)) != Ok(0) {
                        miss!("CBP");
                    }
                } else {
                    miss!("CBP");
                }
            }
            "CBS" => {
                if let Some(view) = buffers.get(&(num(1) as u32)).copied() {
                    if a.buffer_stop(&view, num(1) as u32, num(3)) != Ok(0) {
                        miss!("CBS");
                    }
                }
            }
            "CBG" => {
                if let Some(view) = buffers.get(&(num(1) as u32)).copied() {
                    let ok = a.buffer_get_status(&mut mem, &view, num(1) as u32, 0x6000, num(3));
                    if ok != Ok(0) || mem.words.get(&0x6000).copied() != Some(num(4) as u32) {
                        miss!("CBG");
                    }
                } else if num(4) != 0 {
                    miss!("CBG");
                }
            }
            "CBA" => {
                if a.buffer_pause(num(1) as u32, num(2), num(4) as u32, num(3)) != (num(5) != 0) {
                    miss!("CBA");
                }
            }
            "CBF" => {
                let _ = a.buffer_set_frequency(
                    num(1) as u32,
                    num(2),
                    num(4) as u32,
                    num(5) as u32,
                    num(3),
                );
            }
            "CBL" => {
                // the log carries samples; the adapter takes encoded bytes
                let (ls, le) = (num(4), num(5));
                let _ = a.buffer_set_loop(
                    num(1) as u32,
                    num(2),
                    (ls / 64 * 36) as u32,
                    ((le - ls) / 64 * 36) as u32,
                    num(6) as u32,
                    num(3),
                );
            }
            _ => {}
        }
        // commands produced by this call must equal the logged runtime lines stashed for it, in order
        let produced: Vec<Command> = a.host.log[before..]
            .iter()
            .filter_map(|e| {
                if let Rec::Apply(c, _) = e {
                    Some(c.clone())
                } else {
                    None
                }
            })
            .collect();
        let produced: Vec<Command> = produced
            .into_iter()
            .filter(|c| !matches!(c, Command::ResetBuffers))
            .collect();
        if matches!(op, "CPN" | "CBF") {
            // the C log writes the observation line before the runtime command line of these two
            deferred.extend(produced);
        } else if matches!(op, "CSP" | "CBP" | "CBS" | "CBA") {
            if produced != stash {
                miss!(op);
            }
            stash.clear();
        } else {
            // calls that emit nothing: stashed runtime lines belong to other paths (movie stream), apply as logged
            if !produced.is_empty() {
                miss!(op);
            }
            for c in stash.drain(..) {
                a.host.apply(&c);
            }
        }
        // completion words: every logged delivery wrote its size to the completed address
        for (addr, size, status) in std::mem::take(&mut csd) {
            if (addr != 0 && !mem.writes.contains(&(addr, size)))
                || (status != 0 && !mem.writes.contains(&(status, 0)))
            {
                miss!("CSD");
            }
        }
        for w in expect_writes {
            if w.0 != 0 && !mem.writes.contains(&w) {
                miss!("writes");
            }
        }
        seen = a.host.log.len();
    }
    let _ = seen;
    rep
}

fn other_command(op: &str, f: &[&str]) -> Option<Command> {
    let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    Some(match op {
        "SP" => {
            let b = unhex(f[5]);
            Command::SubmitPcm16Stereo {
                key: n(1) as u32,
                ticks: n(2),
                source_hz: n(3) as u32,
                pcm: b
                    .chunks(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect(),
            }
        }
        "CUT" => Command::CutStream {
            key: n(1) as u32,
            ticks: n(2),
        },
        "VOL" => Command::SetStreamVolume {
            key: n(1) as u32,
            millibels: f[2].parse().ok()?,
        },
        "RVOL" => Command::ResetStreamVolume { key: n(1) as u32 },
        "FRQ" => Command::SetStreamFrequency {
            key: n(1) as u32,
            ticks: n(2),
            source_hz: n(3) as u32,
        },
        "RST" => Command::ResetBuffers,
        "RT" => {
            let mut bins = [0u8; 8];
            let mut bin_volume = [0u32; 32];
            for k in 0..8 {
                bins[k] = n(9 + k) as u8;
            }
            for k in 0..32 {
                bin_volume[k] = n(17 + k) as u32;
            }
            Command::RouteStream {
                key: n(1) as u32,
                serial: n(2),
                ticks: n(3),
                routing: crate::contract::Routing {
                    volume: n(4) as u32,
                    headroom: n(5) as u32,
                    aggregate: n(6) as u32,
                    count: n(7) as u32,
                    bins,
                    bin_volume,
                },
            }
        }
        "FS" => Command::FormatStream {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            old_hz: n(4) as u32,
            new_hz: n(5) as u32,
        },
        "FQ" => Command::FrequencyStream {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            base_hz: n(4) as u32,
            old_hz: n(5) as u32,
            new_hz: n(6) as u32,
        },
        "HE" => Command::EnableHeadroom { enabled: n(1) != 0 },
        "HB" => Command::BindHeadroom { identity: n(1) },
        "HS" => Command::SetHeadroom {
            identity: n(1),
            ticks: n(2),
            bin: n(3) as u32,
            headroom: n(4) as u32,
        },
        "VLB" => Command::VolumeBuffer {
            key: n(1) as u32,
            serial: n(2),
            ticks: n(3),
            volume: f[4].parse().ok()?,
        },
        _ => return None,
    })
}
