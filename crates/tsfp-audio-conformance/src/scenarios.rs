//! Deterministic generated signals: no owner recordings, assets, clocks or devices.
use crate::Step;
use tsfp_audio::contract::{Command, Routing};

pub const OUTPUT_HZ: u32 = 48_000;
pub const TICKS_PER_SECOND: u64 = 48_000;

fn signal(frames: usize, channels: usize) -> Vec<i16> {
    (0..frames * channels)
        .map(|i| ((i as u32 * 997 + 12345) % 60001) as i32 - 30000)
        .map(|s| s as i16)
        .collect()
}

fn render(ticks: u64, capacity: usize) -> Step {
    Step::Render { ticks, capacity }
}
fn cmd(command: Command) -> Step {
    Step::Command(command)
}

fn stream(hz: u32, adpcm: Option<bool>) -> Vec<Step> {
    use Command::*;
    let submit = match adpcm {
        None => SubmitPcm16Stereo {
            key: 1,
            ticks: 0,
            source_hz: hz,
            pcm: signal(2048, 2),
        },
        Some(mono) => {
            let width = if mono { 36 } else { 72 };
            let mut data = vec![0x35; width * 32];
            for block in data.chunks_exact_mut(width) {
                for offset in if mono { vec![0] } else { vec![0, 4] } {
                    block[offset] = 0x20;
                    block[offset + 1] = 0x03;
                    block[offset + 2] = 24;
                    block[offset + 3] = 0;
                }
            }
            SubmitAdpcm {
                key: 1,
                ticks: 0,
                source_hz: hz,
                mono,
                data,
            }
        }
    };
    let route = Routing {
        volume: 0,
        headroom: 0,
        aggregate: 0,
        count: 2,
        bins: [0, 1, 0, 0, 0, 0, 0, 0],
        bin_volume: [0; 32],
    };
    let mut steps = vec![
        cmd(EnableHeadroom { enabled: true }),
        cmd(BindHeadroom { identity: 7 }),
        cmd(RouteStream {
            key: 1,
            serial: 3,
            ticks: 0,
            routing: route,
        }),
        cmd(submit),
        render(128, 128),
        cmd(SetStreamVolume {
            key: 1,
            millibels: -600,
        }),
        cmd(SetStreamVolume {
            key: 1,
            millibels: 1,
        }), // Invalid volume must not mutate.
        cmd(SetHeadroom {
            identity: 7,
            ticks: 128,
            bin: 0,
            headroom: 2,
        }),
        cmd(SetStreamRunning {
            key: 1,
            ticks: 192,
            running: false,
        }),
        cmd(SetStreamRunning {
            key: 1,
            ticks: 256,
            running: true,
        }),
        render(384, 37),
        render(384, 256),
        cmd(SetStreamFrequency {
            key: 1,
            ticks: 384,
            source_hz: hz / 2,
        }),
        render(512, 128),
        cmd(ResetStreamVolume { key: 1 }),
        cmd(FrequencyStream {
            key: 1,
            serial: 3,
            ticks: 512,
            base_hz: hz,
            old_hz: hz / 2,
            new_hz: hz,
        }),
        render(640, 128),
        cmd(FormatStream {
            key: 1,
            serial: 3,
            ticks: 640,
            old_hz: hz,
            new_hz: 48000,
        }),
        cmd(SubmitPcm16Stereo {
            key: 1,
            ticks: 640,
            source_hz: 48000,
            pcm: signal(256, 2),
        }),
        render(768, 128),
        cmd(CutStream { key: 1, ticks: 768 }),
    ];
    // Same timestamp repeatedly drains capacity-limited output, then produces zero frames.
    steps.extend([render(1024, 17), render(1024, 512), render(1024, 32)]);
    steps
}

fn buffer(hz: u32) -> Vec<Step> {
    use Command::*;
    vec![
        cmd(PlayBuffer {
            key: 2,
            serial: 5,
            ticks: 0,
            frequency: hz,
            pcm: signal(512, 1),
            paused: false,
            looping: true,
            loop_start: 16,
            loop_end: 256,
            volume: 0,
        }),
        render(128, 128),
        cmd(VolumeBuffer {
            key: 2,
            serial: 5,
            ticks: 128,
            volume: -1200,
        }),
        cmd(PauseBuffer {
            key: 2,
            serial: 5,
            ticks: 192,
            paused: true,
        }),
        cmd(PauseBuffer {
            key: 2,
            serial: 5,
            ticks: 256,
            paused: false,
        }),
        render(384, 256),
        cmd(FrequencyBuffer {
            key: 2,
            serial: 5,
            ticks: 384,
            frequency: hz / 2,
        }),
        cmd(LoopBuffer {
            key: 2,
            serial: 5,
            ticks: 512,
            loop_start: 32,
            loop_end: 128,
        }),
        render(768, 384),
        cmd(StopBuffer {
            key: 2,
            serial: 5,
            ticks: 768,
        }),
        render(1024, 256),
        cmd(ResetBuffers),
        render(1280, 256),
    ]
}

pub fn fixed_inputs() -> Vec<(String, Vec<Step>)> {
    let mut cases = Vec::new();
    for hz in [8000, 22050, 44100, 48000] {
        for (name, format) in [
            ("pcm", None),
            ("adpcm-mono", Some(true)),
            ("adpcm-stereo", Some(false)),
        ] {
            cases.push((format!("{name}-{hz}"), stream(hz, format)));
        }
        cases.push((format!("buffer-{hz}"), buffer(hz)));
    }
    // Overlapping full-scale streams exercise final saturation and channel separation.
    cases.push((
        "saturation".into(),
        vec![
            cmd(Command::SubmitPcm16Stereo {
                key: 1,
                ticks: 0,
                source_hz: OUTPUT_HZ,
                pcm: vec![30000, -30000].repeat(256),
            }),
            cmd(Command::SubmitPcm16Stereo {
                key: 2,
                ticks: 0,
                source_hz: OUTPUT_HZ,
                pcm: vec![30000, -30000].repeat(256),
            }),
            render(256, 256),
        ],
    ));
    cases
}
