// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 8: line primitives, render targets smaller than the replay surface (texture coordinate scale), the masked clear of a
//! scratch surface and the display transfer option, on FABRICATED streams (no retail bytes). Same stream shapes as `slice5.rs`.
use tsfp_wgpu::replay::Replayer;

#[path = "support/replay.rs"]
mod replay_support;
use replay_support::replay_with;

mod support;
use support::{declaration_hex, hex, vertex};

fn mov_input_to_output(input: u32, address: u32, last: bool) -> [u32; 4] {
    let dword1 = (1 << 21) | (input << 9) | 0x1b;
    let dword2 = 2 << 26;
    let dword3 = (address << 3) | (1 << 11) | (0xF << 12) | u32::from(last);
    [0, dword1, dword2, dword3]
}

fn program_hex() -> String {
    let instructions = [
        mov_input_to_output(1, 0, false),
        mov_input_to_output(2, 3, false),
        mov_input_to_output(3, 9, true),
    ];
    let mut text = format!("{:02x}00", instructions.len());
    for instruction in instructions {
        text.push_str(&hex(&instruction));
    }
    text
}

fn texture_record(address: u32, data: u32, texels: [u32; 4]) -> String {
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    format!(
        "mem {address:x} {}\nmem {:x} {}\n",
        hex(&[0x0004_0001, 0x8000_0000 | data, 0, format_word, 0]),
        0x8000_0000 | data,
        hex(&texels)
    )
}

fn pixel_shader() -> Vec<u32> {
    let mut words = vec![0u32; 0x40];
    words[0x88 / 4] = 0xC4C8_0000;
    words[0xB4 / 4] = 0x0001_00C0;
    words[0x00] = 0xD4D8_1010;
    words[0x68 / 4] = 0x0001_00C0;
    words[0x20 / 4] = 0x0000_0C00;
    words[0x24 / 4] = 0x0000_1C80;
    words[0xD4 / 4] = 0x0001_1101;
    words[0xD8 / 4] = 1;
    words
}

fn size_word(width: u32, height: u32) -> u32 {
    (width - 1) | ((height - 1) << 12) | (39 << 24)
}

fn set_surface(header: u32, data: u32, width: u32, height: u32, flags: u32) -> String {
    format!(
        "call SetSurface {header:x} 0 {data:x} 11229 {:x} {:x} 0 {flags:x}\n",
        size_word(width, height),
        width * 4
    )
}

fn triangle(colour: u32, points: [(f32, f32); 3], uv: [(f32, f32); 3]) -> String {
    (0..3)
        .map(|i| vertex(points[i].0, points[i].1, colour, uv[i].0, uv[i].1))
        .collect()
}

fn preamble() -> String {
    format!(
        "mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         mem 40003000 {}\n\
         call SetPixelShader 40003000\n",
        program_hex(),
        declaration_hex(),
        hex(&pixel_shader())
    )
}

fn white_texture() -> String {
    texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4])
}

#[test]
fn line_primitives_draw_one_pixel_lines() {
    let line: String = [(100.0, 200.5), (400.0, 200.5)]
        .iter()
        .map(|&(x, y)| vertex(x, y, 0xFF00_FF00, 0.0, 0.0))
        .collect();
    let text = format!(
        "{}{}call SetTexture 0 42000100\nmem 40002000 {line}\nhost 19240 40002000 18\ncall DrawVertices 2 0 2\npresent\n",
        preamble(),
        white_texture()
    );
    let Some(lines) = replay_with(&text, |_| {}) else {
        return;
    };
    let frame = &lines.frames[0];
    let column: Vec<[u8; 4]> = (195..206).map(|y| frame.pixel(250, y)).collect();
    assert_eq!(
        &frame.pixel(250, 200)[..3],
        &[0, 255, 0],
        "on the line {column:?}"
    );
    assert_eq!(&frame.pixel(250, 203)[..3], &[0, 0, 0], "beside the line");
    assert_eq!(&frame.pixel(250, 197)[..3], &[0, 0, 0], "beside the line");
    let Some(control) = replay_with(&text, |r| {
        r.knobs.insert("no_lines".into(), "1".into());
    }) else {
        return;
    };
    assert_eq!(
        &control.frames[0].pixel(250, 200)[..3],
        &[0, 0, 0],
        "control: the switch skips line primitives"
    );
}

/// A 160x120 surface (Data 0x2000) painted green over the top left corner of the replay surface, then a triangle on the display
/// sampling it with coordinates in the surface's own texels.
fn small_surface_stream() -> String {
    let paint = triangle(
        0xFF00_FF00,
        [(0.0, 0.0), (400.0, 0.0), (0.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let sampler = triangle(
        0xFF80_8080,
        [(400.0, 100.0), (560.0, 100.0), (400.0, 220.0)],
        [(0.0, 0.0), (160.0, 0.0), (0.0, 120.0)],
    );
    let record = hex(&[
        0x0004_0001,
        0x2000,
        0,
        (0x12 << 8) | 0x29,
        size_word(160, 120),
    ]);
    format!(
        "{}{}{}call SetTexture 0 42000100\nmem 40002000 {paint}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\n{}mem 40002100 {sampler}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n",
        preamble(),
        set_surface(0x4100_0000, 0x2000, 160, 120, 0),
        white_texture(),
        set_surface(0x4100_0100, 0x1000, 640, 480, 1)
    )
}

#[test]
fn a_small_render_target_is_sampled_over_its_whole_extent() {
    // At (520, 110) the surface coordinate is (0.75, 0.08). The replay surface holds the 160x120 image in its top left corner: with
    // the scale the sample is at pixel (120, 10), inside the painted green. Without it the sample is at (480, 40), outside the green
    // triangle (it covers x + y < 400), which is the black of the control.
    let Some(scaled) = replay_with(&small_surface_stream(), |_| {}) else {
        return;
    };
    let inside = scaled.frames[0].pixel(520, 110);
    assert!(
        inside[1] > 200 && inside[0] < 60,
        "far end of the surface shows its green, got {inside:?}"
    );
    let Some(control) = replay_with(&small_surface_stream(), |r| {
        r.knobs.insert("no_surface_uv_scale".into(), "1".into());
    }) else {
        return;
    };
    let outside = control.frames[0].pixel(520, 110);
    assert!(
        outside[1] < 60,
        "control: without the scale the sample is past the painted corner, got {outside:?}"
    );
}

#[test]
fn a_clear_with_colour_mask_zero_keeps_a_scratch_surface() {
    // Same scene, then the scratch surface gets a mask 0 clear with colour (the engine's frame start pattern). With the mask applied
    // (scratch surfaces) the green survives, with the switch the clear would be honoured for the display too.
    let base = small_surface_stream();
    let with_clear = base.replacen(
        "call SetSurface 41000100 ",
        "call PushMethod 40358 0\ncall PushMethod 41d90 ff0000ff\ncall PushMethod 41d94 f3\ncall PushMethod 40358 1010101\ncall SetSurface 41000100 ",
        1,
    );
    assert_ne!(
        base, with_clear,
        "the clear was inserted before the display surface is set"
    );
    let Some(kept) = replay_with(&with_clear, |_| {}) else {
        return;
    };
    let inside = kept.frames[0].pixel(520, 110);
    assert!(
        inside[1] > 200 && inside[2] < 60,
        "scratch surface keeps its green through a mask 0 clear, got {inside:?}"
    );
}

/// T3502: frame one paints the scratch surface green, frame two starts with a mask 0 colour clear (blue) on it and never repaints
/// it before the display samples it. The stale green must not survive into frame two (the effect surface smeared with the camera).
fn stale_scratch_stream() -> String {
    let paint = triangle(
        0xFF00_FF00,
        [(0.0, 0.0), (400.0, 0.0), (0.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let sampler = triangle(
        0xFF80_8080,
        [(400.0, 100.0), (560.0, 100.0), (400.0, 220.0)],
        [(0.0, 0.0), (160.0, 0.0), (0.0, 120.0)],
    );
    let record = hex(&[
        0x0004_0001,
        0x2000,
        0,
        (0x12 << 8) | 0x29,
        size_word(160, 120),
    ]);
    let display = set_surface(0x4100_0100, 0x1000, 640, 480, 1);
    let scratch = set_surface(0x4100_0000, 0x2000, 160, 120, 0);
    let sample = format!(
        "mem 40002100 {sampler}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n"
    );
    format!(
        "{}{scratch}{}call SetTexture 0 42000100\nmem 40002000 {paint}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\n{display}{sample}{scratch}call PushMethod 40358 0\ncall PushMethod 41d90 ff0000ff\ncall PushMethod 41d94 f3\ncall PushMethod 40358 1010101\n{display}{sample}",
        preamble(),
        white_texture(),
    )
}

#[test]
fn a_mask_zero_clear_before_the_first_draw_wipes_stale_scratch_colour() {
    let Some(wiped) = replay_with(&stale_scratch_stream(), |_| {}) else {
        return;
    };
    let Some(first) = wiped.frames.first() else {
        return;
    };
    assert!(
        first.pixel(520, 110)[1] > 200,
        "control: frame one shows the green it painted"
    );
    let pixel = wiped.frames[1].pixel(520, 110);
    assert!(
        pixel[1] < 60,
        "stale green does not accumulate into frame two, got {pixel:?}"
    );
    let Some(kept) = replay_with(&stale_scratch_stream(), |r| {
        r.knobs
            .insert("clear_keeps_stale_scratch".into(), "1".into());
    }) else {
        return;
    };
    let pixel = kept.frames[1].pixel(520, 110);
    assert!(
        pixel[1] > 200,
        "control: the old rule keeps the stale green, got {pixel:?}"
    );
}

#[test]
fn the_display_transfer_option_is_a_no_op_since_t2140() {
    // Slice 8 fitted a display transfer (`host_display`) to a brightness gap that T2140 traced to the missing closing grade pass.
    // The switch is still accepted so old command lines work and it changes nothing. Colour 0x40 times the combiner's x2 is 128.
    let grey = triangle(
        0xFF40_4040,
        [(100.0, 100.0), (300.0, 100.0), (100.0, 300.0)],
        [(0.0, 0.0); 3],
    );
    let text = format!(
        "{}{}call SetTexture 0 42000100\nmem 40002000 {grey}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n",
        preamble(),
        white_texture()
    );
    let Some(plain) = replay_with(&text, |_| {}) else {
        return;
    };
    let Some(shown) = replay_with(&text, |r| {
        r.knobs.insert("host_display".into(), "1".into());
    }) else {
        return;
    };
    assert_eq!(&plain.frames[0].pixel(150, 150)[..3], &[128, 128, 128]);
    assert_eq!(
        plain.frames[0].rgba, shown.frames[0].rgba,
        "the switch changes nothing"
    );
}

#[test]
fn coalesced_texture_uploads_match_control_and_invalidate_generations() {
    let line: String = [(100.0, 200.5), (400.0, 200.5)]
        .iter()
        .map(|&(x, y)| vertex(x, y, 0xFFFF_FFFF, 0.0, 0.0))
        .collect();
    let mut text = preamble();
    for colour in [0xFFFF0000, 0xFF00FF00, 0xFF0000FF] {
        for id in 0..3 {
            text.push_str(&texture_record(
                0x42000100 + id * 0x100,
                0x1100 + id * 0x100,
                [colour; 4],
            ));
            text.push_str(&format!("call SetTexture 0 {:x}\nmem 40002000 {line}\nhost 19240 40002000 18\ncall DrawVertices 2 0 2\n", 0x42000100 + id * 0x100));
        }
        text.push_str("present\n");
    }
    for mips in [false, true] {
        let configure = |r: &mut Replayer| {
            if mips {
                r.knobs.insert("all_mips".into(), "1".into());
            }
        };
        let control = replay_with(&text, configure).expect("lavapipe adapter");
        let batch = replay_with(&text, |r| {
            configure(r);
            r.knobs.insert("batch_textures".into(), "1".into());
        })
        .expect("lavapipe adapter");
        assert_eq!(control.frames.len(), 3);
        for (a, b) in control.frames.iter().zip(&batch.frames) {
            assert_eq!(a.rgba, b.rgba);
        }
        assert_ne!(batch.frames[0].rgba, batch.frames[1].rgba);
        assert_eq!(batch.timings.texture_upload_batches, 3);
        assert_eq!(batch.timings.texture_uploads, 9);
        assert!(batch.timings.texture_staging_bytes > 0);
        assert_eq!(batch.timings.gpu_latency_frames, 3);
    }
}
