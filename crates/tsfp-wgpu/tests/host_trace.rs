// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 2: the text a host run writes with `TSFP_RENDER_TRACE` (crates/tsfp-ports/src/render_trace.rs) must parse and
//! replay. The text here is FABRICATED (a pre-transformed red triangle, no retail bytes) but uses the line shapes of the
//! real trace: comment lines (`# lock`, `# bind ring offset`, `# present`), `host 19240` with the ring data pointer as base,
//! `call` lines with hex words, a `mem` snapshot of the vertex run and `present`.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;

fn vertex_hex(x: f32, y: f32) -> String {
    let words = [x.to_bits(), y.to_bits(), 0.5f32.to_bits(), 0xFFFF_0000];
    words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn host_trace_text() -> String {
    let mem = format!(
        "{}{}{}",
        vertex_hex(100.0, 100.0),
        vertex_hex(300.0, 100.0),
        vertex_hex(100.0, 300.0)
    );
    format!(
        "# tsfp-wgpu stream v1 (T2024 slice 2, host render trace)\n\
         call LockVertexBuffer 40fca030 30\n\
         # lock 40fca030 -> 421aa000\n\
         # bind ring offset 201\n\
         host 19240 421aa000 10\n\
         call SetPixelShader 406f3e20\n\
         mem 421aa000 {mem}\n\
         call DrawVertices 5 0 3\n\
         # present 1\n\
         present\n"
    )
}

#[test]
fn host_trace_text_parses() {
    let stream = Stream::from_text(&host_trace_text()).expect("host trace parses");
    let histogram = stream.call_histogram();
    assert_eq!(histogram.get("DrawVertices"), Some(&1));
    assert_eq!(histogram.get("LockVertexBuffer"), Some(&1));
    assert_eq!(histogram.get("SetPixelShader"), Some(&1));
}

#[test]
fn host_trace_text_replays_a_visible_triangle() {
    let gpu = match GpuContext::new(&Preference::Software) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no software adapter): {error}");
            return;
        }
    };
    let stream = Stream::from_text(&host_trace_text()).expect("host trace parses");
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(&stream);
    assert_eq!(replayer.frames.len(), 1);
    let frame = &replayer.frames[0];
    let inside = frame.pixel(150, 150);
    assert_eq!(
        &inside[..3],
        &[255, 0, 0],
        "triangle interior is red: {inside:?}"
    );
    let outside = frame.pixel(400, 400);
    assert_eq!(
        &outside[..3],
        &[0, 0, 0],
        "outside stays the clear colour: {outside:?}"
    );
}
