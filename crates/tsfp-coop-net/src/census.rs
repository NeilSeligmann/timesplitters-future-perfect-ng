// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic transport census. This integer toy simulation is FABRICATED and
//! cannot establish game determinism. Both peers apply inputs in global order.
use crate::Channel;
use tsfp_input::player::{AssistMask, Look, MenuIntent, PlayerInput, SourceKind, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct Impairment {
    pub seed: u64,
    pub loss_percent: u8,
    pub duplicate_percent: u8,
    pub latency_ticks: u64,
    pub jitter_ticks: u64,
    pub input_delay: u64,
}
impl Default for Impairment {
    fn default() -> Self {
        Self {
            seed: 2153,
            loss_percent: 0,
            duplicate_percent: 0,
            latency_ticks: 0,
            jitter_ticks: 0,
            input_delay: 8,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Measurement {
    pub frames: u64,
    pub first_desync: Option<u64>,
    pub missing_inputs: [u64; 2],
    pub final_hashes: [u64; 2],
}

/// Reproducible pad-like stream, not disc input or an original game fixture.
pub fn synthetic_input(step: u64, slot: u8) -> PlayerInput {
    PlayerInput {
        step,
        slot,
        source: SourceKind::Script,
        held: ((step * 17 + u64::from(slot) * 13) & 0xffff) as u32,
        pressed: 0,
        released: 0,
        move_axis: Vec2::new(((step + u64::from(slot)) % 3) as f32 - 1.0, 0.0),
        look: Look::Stick(Vec2::ZERO),
        triggers: [0.0; 2],
        menu: MenuIntent::default(),
        assists: AssistMask::NONE,
    }
}

struct Flight {
    due: u64,
    target: usize,
    bytes: Vec<u8>,
    order: u64,
}
struct Shim {
    config: Impairment,
    random: u64,
    packets: Vec<Flight>,
}
impl Shim {
    fn random(&mut self) -> u64 {
        self.random = self
            .random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.random >> 32
    }
    fn send(&mut self, tick: u64, target: usize, bytes: Vec<u8>) {
        if self.random() % 100 < u64::from(self.config.loss_percent) {
            return;
        }
        let copies = if self.random() % 100 < u64::from(self.config.duplicate_percent) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let jitter = self.random() % (self.config.jitter_ticks + 1);
            let order = self.random();
            self.packets.push(Flight {
                due: tick + self.config.latency_ticks + jitter,
                target,
                bytes: bytes.clone(),
                order,
            });
        }
    }
    fn deliver(&mut self, tick: u64, channels: &mut [Channel; 2]) {
        self.packets.sort_by_key(|p| (p.due, p.order));
        let count = self.packets.partition_point(|p| p.due <= tick);
        let ready: Vec<_> = self.packets.drain(..count).collect();
        for packet in ready {
            channels[packet.target]
                .receive(&packet.bytes)
                .expect("shim emits valid input packets");
        }
    }
}

/// Replay two local synthetic instances through the actual input codec/channel.
/// A missing frame uses neutral input explicitly; no hidden hold-last state.
/// `perturb_at` flips one peer's simulation state AFTER applying that frame,
/// proving that the detector reports onset rather than merely final inequality.
pub fn measure(frames: u64, config: Impairment, perturb_at: Option<u64>) -> Measurement {
    assert!(config.loss_percent <= 100 && config.duplicate_percent <= 100);
    // Bound arithmetic and memory use in this offline diagnostic.
    assert!(
        frames <= 1_000_000
            && config.latency_ticks <= 1000
            && config.jitter_ticks <= 1000
            && config.input_delay <= 1000
    );
    let mut channels = [Channel::new(2153, 20), Channel::new(2153, 20)];
    let mut shim = Shim {
        config,
        random: config.seed,
        packets: Vec::new(),
    };
    let mut result = Measurement {
        frames,
        first_desync: None,
        missing_inputs: [0; 2],
        final_hashes: [0; 2],
    };
    // Send two additional inputs to flush redundancy for the last measured frame.
    for tick in 0..frames + config.input_delay + 2 {
        if tick < frames + 2 {
            for (sender, channel) in channels.iter_mut().enumerate() {
                let bytes = channel.input(synthetic_input(tick, sender as u8)).unwrap();
                shim.send(tick, 1 - sender, bytes);
            }
        }
        shim.deliver(tick, &mut channels);
        if tick < config.input_delay || tick - config.input_delay >= frames {
            continue;
        }
        let step = tick - config.input_delay;
        for (peer, channel) in channels.iter_mut().enumerate() {
            let remote = channel.take_input(step);
            if remote.is_none() {
                result.missing_inputs[peer] += 1;
            }
            for slot in 0..2 {
                let input = if slot == peer {
                    Some(synthetic_input(step, slot as u8))
                } else {
                    remote
                };
                let value = input.map_or(0, |p| {
                    u64::from(p.held) + ((p.move_axis.x + 1.0) as u64) * 65536
                });
                result.final_hashes[peer] = result.final_hashes[peer]
                    .wrapping_mul(1099511628211)
                    .wrapping_add(value);
            }
        }
        if perturb_at == Some(step) {
            result.final_hashes[1] ^= 1;
        }
        if result.final_hashes[0] != result.final_hashes[1] && result.first_desync.is_none() {
            result.first_desync = Some(step);
        }
    }
    result
}
