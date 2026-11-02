//! T2309: UI batches recorded into the engine frame through `tsfp_wgpu::frame_layer`.
//! FABRICATED scene (`scenes::title_scene`) and batches, MEASURED software-GPU output.
//! One test function: the frame layer is process-global, so the cases run in order.
use std::sync::{Arc, Mutex};
use tsfp_ui_renderer::*;
use tsfp_ui_wgpu::WgpuUiRenderer;
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::frame_layer::{self, FrameLayer, LayerTarget};
use tsfp_wgpu::replay::{Frame, Replayer};

mod support;
use support::rect;

/// Draws one additive quad and one alpha quad at fixed pixels; records the target sizes it saw.
struct TwoQuads {
    seen: Arc<Mutex<Vec<(u32, u32)>>>,
    renderer: Option<WgpuUiRenderer<'static>>,
}

const ADD: Color = Color([0.5, 0.25, 0.0, 0.5]);
const OVER: Color = Color([1.0, 0.0, 1.0, 1.0]);

impl FrameLayer for TwoQuads {
    fn encode(&mut self, t: LayerTarget<'_>) -> bool {
        let (w, h) = (t.target.width(), t.target.height());
        self.seen.lock().unwrap().push((w, h));
        let renderer = match &mut self.renderer {
            Some(r) => {
                r.retarget(t.target).unwrap();
                r
            }
            None => self
                .renderer
                .insert(WgpuUiRenderer::new_owned(t.device, t.queue, t.target).unwrap()),
        };
        let viewport = Viewport::new(
            Vec2::new(w as f32, h as f32),
            1.0,
            rect(0., 0., w as f32, h as f32),
        )
        .unwrap();
        renderer.begin_frame(viewport).unwrap();
        renderer
            .submit(&[
                Batch::Blend(BlendMode::Additive),
                Batch::Quads(&[Quad {
                    rect_px: rect(2., 2., 4., 4.),
                    color: ADD,
                }]),
                Batch::Blend(BlendMode::Alpha),
                Batch::Quads(&[Quad {
                    rect_px: rect(10., 2., 2., 2.),
                    color: OVER,
                }]),
            ])
            .unwrap();
        renderer.end_frame_into(t.encoder).unwrap();
        true
    }
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Frame {
    let pitch = (texture.width() * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("in-frame-readback"),
        size: (pitch * texture.height()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
    device.poll(wgpu::Maintain::Wait);
    receiver.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let rgba = mapped
        .chunks_exact(pitch as usize)
        .flat_map(|row| row[..texture.width() as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    Frame {
        width: texture.width(),
        height: texture.height(),
        rgba,
    }
}

/// One title scene frame at `output`, optionally through the window present pass.
fn scene(window: Option<(u32, u32)>) -> (Frame, Replayer) {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let mut replay = Replayer::new(gpu, 640, 480);
    replay.set_output_size(64, 32);
    replay.set_window_size(window);
    replay.run(&tsfp_wgpu::scenes::title_scene());
    let (device, queue) = replay.device_and_queue();
    let frame = read(device, queue, replay.display_texture().unwrap());
    (frame, replay)
}

fn close(a: u8, b: f32) -> bool {
    (f32::from(a) - b).abs() <= 1.5
}

fn check_composited(base: &Frame, after: &Frame) {
    assert_eq!((base.width, base.height), (after.width, after.height));
    let mut changed = 0;
    for y in 0..base.height {
        for x in 0..base.width {
            let (b, a) = (base.pixel(x, y), after.pixel(x, y));
            if (2..6).contains(&x) && (2..6).contains(&y) {
                // Additive: dst + src * alpha per colour channel, destination alpha kept.
                for c in 0..3 {
                    let want = (f32::from(b[c]) + ADD.0[c] * ADD.0[3] * 255.0).min(255.0);
                    assert!(close(a[c], want), "additive ({x},{y}) c{c}: {a:?} from {b:?}");
                }
                assert_eq!(a[3], b[3], "additive keeps alpha at ({x},{y})");
                changed += 1;
            } else if (10..12).contains(&x) && (2..4).contains(&y) {
                assert_eq!(&a[..3], &[255, 0, 255], "opaque alpha quad at ({x},{y})");
            } else {
                assert_eq!(a, b, "scene pixel ({x},{y}) untouched");
            }
        }
    }
    assert_eq!(changed, 16);
}

#[test]
fn ui_batches_are_composited_inside_the_engine_frame() {
    // Guest-surface path (no window present pass): the layer draws on a private composite.
    let (base, _) = scene(None);
    assert!(
        base.rgba.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]),
        "the scene draws something (non-empty baseline)"
    );
    let seen = Arc::new(Mutex::new(Vec::new()));
    frame_layer::install("test", 0, Box::new(TwoQuads {
        seen: seen.clone(),
        renderer: None,
    }));
    let (after, replay) = scene(None);
    assert_eq!(replay.layer_drawn, 1);
    check_composited(&base, &after);
    // Window present pass path: the layer draws on the window sized target, after the present pass.
    frame_layer::remove("test");
    let (base_window, _) = scene(Some((96, 32)));
    frame_layer::install("test", 0, Box::new(TwoQuads {
        seen: seen.clone(),
        renderer: None,
    }));
    let (after_window, _) = scene(Some((96, 32)));
    check_composited(&base_window, &after_window);
    assert_eq!(*seen.lock().unwrap(), [(64, 32), (96, 32)]);
    // Removed: frames are presented untouched again.
    frame_layer::remove("test");
    let (again, replay) = scene(None);
    assert_eq!(again.rgba, base.rgba);
    assert_eq!(replay.layer_drawn, 0);
}
