// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 4: second texture stage, render target copy and repeat addressing, on FABRICATED streams (no retail bytes). Same
//! stream shapes as `vertex_program.rs`.

#[path = "support/replay.rs"]
mod replay_support;
use replay_support::replay_with;

mod support;
use support::{declaration_hex, hex, vertex};

#[path = "support/textured_stream.rs"]
mod textured_stream;
use textured_stream::{pixel_shader, program_hex, texture_record};

#[test]
fn second_texture_stage_is_sampled_by_the_combiner() {
    // texture modes 0x21 (stages 0 and 1), colour = tex0 x tex1 x 2
    let words = pixel_shader(0xC8C9_0000, 0xD430_1010, 0x21);
    let vertices = format!(
        "{}{}{}",
        vertex(100.0, 100.0, 0xFF80_8080, 0.0, 0.0),
        vertex(400.0, 100.0, 0xFF80_8080, 1.0, 0.0),
        vertex(100.0, 400.0, 0xFF80_8080, 0.0, 1.0)
    );
    let text = format!(
        "mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         host 19240 40002000 18\n\
         mem 40002000 {vertices}\n\
         mem 40003000 {}\n\
         call SetPixelShader 40003000\n\
         {}{}\
         call SetTexture 0 42000000\n\
         call SetTexture 1 42000100\n\
         call DrawVertices 5 0 3\n\
         present\n",
        program_hex(true),
        declaration_hex(),
        hex(&words),
        texture_record(0x4200_0000, 0x1000, [0xFF80_8080; 4]),
        texture_record(0x4200_0100, 0x1100, [0xFF40_C040; 4])
    );
    let Some(replayer) = replay_with(&text, |_| {}) else {
        return;
    };
    // 0.502 x 0.251 x 2 = 0.252 (64), 0.502 x 0.753 x 2 = 0.756 (193). With stage 1 unbound it would read white (255).
    let inside = replayer.frames[0].pixel(150, 150);
    assert!(
        (60..=68).contains(&inside[0])
            && (188..=198).contains(&inside[1])
            && (60..=68).contains(&inside[2]),
        "interior {inside:?}"
    );
}

fn framebuffer_stream() -> String {
    // Draw A paints a green triangle. Draw B (right side, still black) samples a linear 640x480 record without texel data, with
    // texel coordinates inside the green triangle.
    let words = pixel_shader(0xC4C8_0000, 0xD4D8_1010, 1);
    let vertices_a = format!(
        "{}{}{}",
        vertex(100.0, 100.0, 0xFF00_FF00, 0.0, 0.0),
        vertex(400.0, 100.0, 0xFF00_FF00, 0.0, 0.0),
        vertex(100.0, 400.0, 0xFF00_FF00, 0.0, 0.0)
    );
    let vertices_b = format!(
        "{}{}{}",
        vertex(450.0, 100.0, 0xFF80_8080, 100.0, 100.0),
        vertex(550.0, 100.0, 0xFF80_8080, 200.0, 100.0),
        vertex(450.0, 200.0, 0xFF80_8080, 100.0, 200.0)
    );
    let size_word = 639 | (479 << 12) | (39 << 24);
    let record = hex(&[0x0004_0001, 0x8000_4000, 0, (0x12 << 8) | 0x29, size_word]);
    format!(
        "mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         mem 40002000 {vertices_a}\n\
         host 19240 40002000 18\n\
         call DrawVertices 5 0 3\n\
         mem 40002100 {vertices_b}\n\
         host 19240 40002100 18\n\
         mem 40003000 {}\n\
         call SetPixelShader 40003000\n\
         mem 42000000 {record}\n\
         call SetTexture 0 42000000\n\
         call DrawVertices 5 0 3\n\
         present\n",
        program_hex(false),
        declaration_hex(),
        hex(&words)
    )
}

#[test]
fn draw_that_samples_a_frame_buffer_surface_reads_a_copy_of_the_target() {
    let Some(replayer) = replay_with(&framebuffer_stream(), |_| {}) else {
        return;
    };
    let frame = &replayer.frames[0];
    assert_eq!(&frame.pixel(150, 150)[..3], &[0, 255, 0], "draw A");
    let copied = frame.pixel(470, 120);
    assert!(
        copied[0] < 30 && copied[1] > 225 && copied[2] < 30,
        "draw B reads the green copy, got {copied:?}"
    );
}

#[test]
fn skip_switch_leaves_the_frame_buffer_draw_out() {
    let Some(skipped) = replay_with(&framebuffer_stream(), |r| r.skip_framebuffer_samples = true)
    else {
        return;
    };
    assert_eq!(&skipped.frames[0].pixel(470, 120)[..3], &[0, 0, 0]);
}

#[test]
fn repeat_addressing_wraps_texture_coordinates_past_one() {
    // left column red, right column green. u runs 1.0 to 2.0 over x 100..400, v is 0.25 (top row centre). At x = 175, u = 1.25:
    // repeat gives the red texel centre, clamp the green edge.
    let vertex_uv = |x: f32, y: f32| vertex(x, y, 0xFFFF_FFFF, 1.0 + (x - 100.0) / 300.0, 0.25);
    let vertices = format!(
        "{}{}{}",
        vertex_uv(100.0, 100.0),
        vertex_uv(400.0, 100.0),
        vertex_uv(100.0, 400.0)
    );
    let text = format!(
        "mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         host 19240 40002000 18\n\
         mem 40002000 {vertices}\n\
         {}\
         call SetTexture 0 42000000\n\
         call DrawVertices 5 0 3\n\
         present\n",
        program_hex(false),
        declaration_hex(),
        texture_record(
            0x4200_0000,
            0x1000,
            [0xFFFF_0000, 0xFF00_FF00, 0xFFFF_0000, 0xFF00_FF00]
        )
    );
    let Some(wrapped) = replay_with(&text, |r| r.wrap_textures = true) else {
        return;
    };
    let red = wrapped.frames[0].pixel(175, 110);
    assert!(red[0] > 230 && red[1] < 40, "repeat {red:?}");
    let Some(clamped) = replay_with(&text, |_| {}) else {
        return;
    };
    let green = clamped.frames[0].pixel(175, 110);
    assert!(green[1] > 230 && green[0] < 40, "clamp {green:?}");
}
