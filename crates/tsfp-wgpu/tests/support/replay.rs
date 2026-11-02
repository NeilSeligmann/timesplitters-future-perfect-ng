// SPDX-License-Identifier: GPL-3.0-or-later
//! Private software replay setup for synthetic renderer conformance suites.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;

/// Configure a 640x480 replay before applying the stream; skip unavailable adapters.
pub fn replay_with(text: &str, configure: impl FnOnce(&mut Replayer)) -> Option<Replayer> {
    let gpu = match GpuContext::new(&Preference::Software) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no software adapter): {error}");
            return None;
        }
    };
    let stream = Stream::from_text(text).expect("stream parses");
    let mut replayer = Replayer::new(gpu, 640, 480);
    configure(&mut replayer);
    replayer.run(&stream);
    Some(replayer)
}
