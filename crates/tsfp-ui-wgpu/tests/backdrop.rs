//! T2313: the page fill's back-buffer pass (`0x204F0` stand-in) on a FABRICATED stripe target,
//! MEASURED software-GPU output (Tier 0): blurred and darkened inside the rounded box only, the
//! frame's alpha kept, nothing outside touched, strength 0 records nothing.
use tsfp_ui_wgpu::backdrop::{BackdropParams, BackdropPass};
use tsfp_wgpu::context::{GpuContext, Preference};

mod support;
use support::rect;

const SIZE: (u32, u32) = (64, 48);

/// One-pixel vertical stripes 0 / 200, alpha 77 (guest render state the pass must keep).
fn stripes() -> Vec<u8> {
    let mut rgba = Vec::new();
    for _y in 0..SIZE.1 {
        for x in 0..SIZE.0 {
            let v = if x % 2 == 0 { 0 } else { 200 };
            rgba.extend_from_slice(&[v, v, v, 77]);
        }
    }
    rgba
}

fn upload(gpu: &GpuContext, texture: &wgpu::Texture, rgba: &[u8]) {
    gpu.queue.write_texture(
        texture.as_image_copy(),
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SIZE.0 * 4),
            rows_per_image: Some(SIZE.1),
        },
        texture.size(),
    );
}

fn read(gpu: &GpuContext, texture: &wgpu::Texture) -> Vec<u8> {
    let pitch = (SIZE.0 * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("backdrop-readback"),
        size: u64::from(pitch * SIZE.1),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(SIZE.1),
            },
        },
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
    gpu.device.poll(wgpu::Maintain::Wait);
    receiver.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range();
    let rgba = mapped
        .chunks_exact(pitch as usize)
        .flat_map(|row| row[..SIZE.0 as usize * 4].iter().copied())
        .collect();
    drop(mapped);
    buffer.unmap();
    rgba
}

fn run(gpu: &GpuContext, pass: &mut BackdropPass, params: BackdropParams) -> (Vec<u8>, bool) {
    let texture = support::target(gpu, "backdrop", SIZE, [0, 0, 0, 0]);
    upload(gpu, &texture, &stripes());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let recorded = pass.encode(&gpu.device, &gpu.queue, &mut encoder, &texture, params);
    gpu.queue.submit([encoder.finish()]);
    (read(gpu, &texture), recorded)
}

#[test]
fn backdrop_blurs_and_darkens_inside_the_rounded_box_only() {
    let gpu = GpuContext::new(&Preference::Software).expect("software GPU required");
    let mut pass = BackdropPass::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let params = BackdropParams {
        rect: rect(16.0, 8.0, 32.0, 32.0),
        radius: 8.0,
        sigma: 2.0,
        darken: 8.0 / 255.0,
        strength: 1.0,
    };
    let base = stripes();
    let (after, recorded) = run(&gpu, &mut pass, params);
    assert!(recorded);
    let at = |rgba: &[u8], x: u32, y: u32| {
        let i = ((y * SIZE.0 + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    let mut inside = 0;
    for y in 0..SIZE.1 {
        for x in 0..SIZE.0 {
            let (b, a) = (at(&base, x, y), at(&after, x, y));
            assert_eq!(a[3], 77, "alpha kept at ({x},{y})");
            let in_rect = (16..48).contains(&x) && (8..40).contains(&y);
            if !in_rect {
                assert_eq!(a, b, "outside the box untouched at ({x},{y})");
            } else if (20..44).contains(&x) && (12..36).contains(&y) {
                // Stripes average 100, minus the 8-level darkening.
                for c in 0..3 {
                    assert!(
                        (i32::from(a[c]) - 92).abs() <= 3,
                        "blurred and darkened at ({x},{y}): {a:?}"
                    );
                }
                inside += 1;
            }
        }
    }
    assert_eq!(inside, 24 * 24);
    // The rounded corner: the rect's corner pixel lies outside the radius-8 arc.
    assert_eq!(at(&after, 16, 8), at(&base, 16, 8));
    assert_eq!(at(&after, 47, 39), at(&base, 47, 39));

    // Half strength (page fading in): halfway between the scene and the full pass.
    let (half, _) = run(&gpu, &mut pass, BackdropParams { strength: 0.5, ..params });
    for (x, want) in [(30u32, (0.0 + 92.0) / 2.0), (31, (200.0 + 92.0) / 2.0)] {
        let got = f32::from(at(&half, x, 24)[0]);
        assert!((got - want).abs() <= 3.0, "half strength at x {x}: {got} want {want}");
    }

    // Strength 0 (page faded out) records nothing.
    let (none, recorded) = run(&gpu, &mut pass, BackdropParams { strength: 0.0, ..params });
    assert!(!recorded);
    assert_eq!(none, base);
}
