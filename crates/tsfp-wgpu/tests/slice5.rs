// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 5: render to texture from `SetSurface`, the library push trace (`lib`) and the colour write mask, on FABRICATED
//! streams (no retail bytes). Same stream shapes as `slice4.rs`.
//! stream shapes as `vertex_program.rs`.
use tsfp_wgpu::live::{LiveConfig, LiveRenderer};
use tsfp_wgpu::stream::Stream;

#[path = "support/replay.rs"]
mod replay_support;
use replay_support::replay_with;

mod support;
use support::{declaration_hex, hex, vertex};

#[path = "support/textured_stream.rs"]
mod textured_stream;
use textured_stream::{pixel_shader, program_hex, texture_record};

const SIZE_640X480: u32 = 639 | (479 << 12) | (39 << 24);

/// `call SetSurface` for a 640x480 linear surface whose Data word is `data` (`flags` bit 0: the display surface).
fn set_surface(header: u32, data: u32, flags: u32) -> String {
    format!("call SetSurface {header:x} 0 {data:x} 11229 {SIZE_640X480:x} a00 0 {flags:x}\n")
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
        program_hex(false),
        declaration_hex(),
        hex(&pixel_shader(0xC4C8_0000, 0xD4D8_1010, 1))
    )
}

#[test]
fn render_to_texture_samples_the_surface_another_target_drew() {
    // Surface 0x2000 (not the display) receives a green triangle. The display surface 0x1000 then draws a quad that samples a
    // texture record whose Data word is 0x2000: it shows the green, and the triangle itself is not on the display.
    let a = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let b = triangle(
        0xFF80_8080,
        [(450.0, 100.0), (550.0, 100.0), (450.0, 200.0)],
        [(100.0, 100.0), (200.0, 100.0), (100.0, 200.0)],
    );
    let record = hex(&[0x0004_0001, 0x2000, 0, (0x12 << 8) | 0x29, SIZE_640X480]);
    let text = format!(
        "{}{}{}call SetTexture 0 42000100\nmem 40002000 {a}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\n{}mem 40002100 {b}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n",
        preamble(),
        set_surface(0x4100_0000, 0x2000, 0),
        texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4]),
        set_surface(0x4100_0100, 0x1000, 1)
    );
    let Some(replayer) = replay_with(&text, |r| {
        r.fb_snapshot = tsfp_wgpu::replay::FbSnapshot::PerDraw
    }) else {
        return;
    };
    let frame = &replayer.frames[0];
    assert_eq!(
        &frame.pixel(150, 150)[..3],
        &[0, 0, 0],
        "the triangle is on the other surface"
    );
    let copied = frame.pixel(470, 120);
    assert!(
        copied[0] < 30 && copied[1] > 225 && copied[2] < 30,
        "the quad reads the green surface, got {copied:?}"
    );
}

#[test]
fn rendering_into_the_sampled_surface_reads_a_copy_taken_before_the_draw() {
    // One surface: draw A paints green, draw B samples the same surface (feedback): the copy has A, B shows green.
    let a = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let b = triangle(
        0xFF80_8080,
        [(450.0, 100.0), (550.0, 100.0), (450.0, 200.0)],
        [(100.0, 100.0), (200.0, 100.0), (100.0, 200.0)],
    );
    let record = hex(&[0x0004_0001, 0x1000, 0, (0x12 << 8) | 0x29, SIZE_640X480]);
    let text = format!(
        "{}{}{}call SetTexture 0 42000100\nmem 40002000 {a}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\nmem 40002100 {b}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n",
        preamble(),
        set_surface(0x4100_0100, 0x1000, 1),
        texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4])
    );
    let Some(replayer) = replay_with(&text, |_| {}) else {
        return;
    };
    let copied = replayer.frames[0].pixel(470, 120);
    assert!(
        copied[1] > 225 && copied[0] < 30,
        "feedback copy {copied:?}"
    );
}

/// A green triangle with the library push `lib 358 <mask>` AFTER the draw call, as the traced host orders them.
fn mask_stream(mask: Option<u32>) -> String {
    let a = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let lib = mask.map_or(String::new(), |value| format!("lib 358 {value:x}\n"));
    format!(
        "{}mem 40002000 {a}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\n{lib}present\n",
        preamble()
    )
}

#[test]
fn colour_write_mask_pushed_after_the_draw_call_applies_to_that_draw() {
    let Some(open) = replay_with(&mask_stream(None), |_| {}) else {
        return;
    };
    assert_eq!(
        &open.frames[0].pixel(150, 150)[..3],
        &[0, 255, 0],
        "control: no mask"
    );
    // red only: the green triangle writes nothing visible
    let Some(masked) = replay_with(&mask_stream(Some(0x1_0000)), |_| {}) else {
        return;
    };
    assert_eq!(
        &masked.frames[0].pixel(150, 150)[..3],
        &[0, 0, 0],
        "red only mask"
    );
    // blue only keeps the black too, and the same mask with the experiment switch off shows the draw
    let Some(ignored) = replay_with(&mask_stream(Some(0x1_0000)), |r| {
        r.knobs.insert("no_colour_mask".into(), "1".into());
    }) else {
        return;
    };
    assert_eq!(
        &ignored.frames[0].pixel(150, 150)[..3],
        &[0, 255, 0],
        "switch off"
    );
}

#[test]
fn texture_address_words_from_the_library_choose_wrap_or_clamp() {
    // left column red, right column green, u 1.0 to 2.0: wrap gives red at x = 175, clamp gives green.
    let vertex_uv = |x: f32, y: f32| vertex(x, y, 0xFFFF_FFFF, 1.0 + (x - 100.0) / 300.0, 0.25);
    let vertices = format!(
        "{}{}{}",
        vertex_uv(100.0, 100.0),
        vertex_uv(400.0, 100.0),
        vertex_uv(100.0, 400.0)
    );
    let stream = |address: u32| {
        format!(
            "mem 40000002 {}\n\
             call LoadVertexProgram 40000000 0\n\
             mem 40001000 {}\n\
             call SelectVertexDeclaration 40001000 0\n\
             host 19240 40002000 18\n\
             mem 40002000 {vertices}\n\
             {}\
             call SetTexture 0 42000000\n\
             call DrawVertices 5 0 3\n\
             lib 1b08 {address:x}\n\
             present\n",
            program_hex(false),
            declaration_hex(),
            texture_record(
                0x4200_0000,
                0x1000,
                [0xFFFF_0000, 0xFF00_FF00, 0xFFFF_0000, 0xFF00_FF00]
            )
        )
    };
    let Some(wrapped) = replay_with(&stream(0x0101), |_| {}) else {
        return;
    };
    let red = wrapped.frames[0].pixel(175, 110);
    assert!(red[0] > 230 && red[1] < 40, "wrap {red:?}");
    let Some(clamped) = replay_with(&stream(0x0303), |r| r.wrap_textures = true) else {
        return;
    };
    let green = clamped.frames[0].pixel(175, 110);
    assert!(
        green[1] > 230 && green[0] < 40,
        "clamp even with the heuristic on {green:?}"
    );
}

#[test]
fn surface_and_library_events_round_trip_through_the_text_form() {
    let text = "call SetSurface 41000100 3e59cc 374000 11229 271df27f a00 5cc000 1\nlib 358 10101\npresent\n";
    let stream = Stream::from_text(text).expect("parses");
    assert_eq!(stream.events.len(), 3);
    let again = stream.to_text();
    assert!(again.contains("call SetSurface 41000100 3e59cc 374000 11229 271df27f a00 5cc000 1"));
    assert!(again.contains("lib 358 10101"));
    assert_eq!(
        Stream::from_text(&again).expect("reparses").events,
        stream.events
    );
}

#[test]
fn alpha_only_clear_keeps_the_colour_channels() {
    // CLEAR_SURFACE 0x80 clears alpha only (the engine does this between post process passes): the green triangle survives a
    // clear whose colour is blue. A full colour clear (0xF0) wipes it to blue.
    let a = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let stream = |flags: u32| {
        format!(
            "{}{}{}call SetTexture 0 42000100\nmem 40002000 {a}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\ncall PushMethod 41d90 ff0000ff\ncall PushMethod 41d94 {flags:x}\npresent\n",
            preamble(),
            set_surface(0x4100_0100, 0x1000, 1),
            texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4])
        )
    };
    let Some(partial) = replay_with(&stream(0x80), |_| {}) else {
        return;
    };
    assert_eq!(
        &partial.frames[0].pixel(150, 150)[..3],
        &[0, 255, 0],
        "alpha only clear"
    );
    let Some(full) = replay_with(&stream(0xF0), |_| {}) else {
        return;
    };
    assert_eq!(
        &full.frames[0].pixel(150, 150)[..3],
        &[0, 0, 255],
        "full clear"
    );
}

/// A dropped frame is not semantically disposable when another frame reads its persistent target.
/// This is a synthetic counterexample, not a claim about frequency in retail routes.
#[test]
fn dropped_frame_loses_persistent_render_to_texture_feedback() {
    dropped_feedback(false);
}
#[test]
fn suppressed_presentation_preserves_persistent_clear_feedback() {
    dropped_feedback(true);
}
fn dropped_feedback(clear: bool) {
    let a = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let b = triangle(
        0xFF80_8080,
        [(450.0, 100.0), (550.0, 100.0), (450.0, 200.0)],
        [(100.0, 100.0), (200.0, 100.0), (100.0, 200.0)],
    );
    let first = format!(
        "{}{}{}call SetTexture 0 42000100\nmem 40002000 {a}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n",
        preamble(),
        set_surface(0x4100_0000, 0x2000, 0),
        texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4]),
    );
    let first = if clear {
        first.replace(
            "call DrawVertices 5 0 3",
            "call PushMethod 41d90 ff00ff00\ncall PushMethod 41d94 f0",
        )
    } else {
        first
    };
    let record = hex(&[0x0004_0001, 0x2000, 0, (0x12 << 8) | 0x29, SIZE_640X480]);
    let second = format!(
        "{}mem 40002100 {b}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n",
        set_surface(0x4100_0100, 0x1000, 1),
    );
    let Some(mut full) = replay_with(&first, |_| {}) else {
        return;
    };
    let mut live = LiveRenderer::new(LiveConfig::software()).unwrap();
    live.set_skip_draw(true);
    for line in first.lines() {
        live.feed_line(line).unwrap();
    }
    assert!(live.last_frame().is_none());
    live.set_skip_draw(false);
    for line in second.lines() {
        live.feed_line(line).unwrap();
    }
    let Some(mut dropped) = replay_with(&first, |r| r.rendering = false) else {
        return;
    };
    let Some(mut preserved) = replay_with(&first, |r| {
        r.knobs.insert("pipelined_readback".into(), "1".into());
        r.suppress_presentation = true;
    }) else {
        return;
    };
    let second = Stream::from_text(&second).unwrap();
    // Switch back to synchronous readback to compare the next frame exactly.
    preserved.knobs.clear();
    preserved.suppress_presentation = false;
    preserved.run(&second);
    full.run(&second);
    assert_eq!(
        full.frames.last().unwrap().rgba,
        preserved.frames.last().unwrap().rgba
    );
    assert_eq!(
        full.frames.last().unwrap().rgba,
        live.last_frame().unwrap().rgba
    );
    dropped.rendering = true;
    dropped.run(&second);
    let expected = full.frames.last().unwrap();
    let actual = dropped.frames.last().unwrap();
    assert!(
        expected.pixel(470, 120)[1] > 225,
        "undropped reference must read green"
    );
    assert_eq!(&actual.pixel(470, 120)[..3], &[0, 0, 0]);
    assert_ne!(
        expected.rgba, actual.rgba,
        "dropping persistent work is observably incorrect"
    );
    // State-only skipping did apply the same binding and memory records.
    assert_eq!(full.lib_regs, dropped.lib_regs);
}

/// T2294: a suppressed present after a run of frames that each start with a full colour clear omits the draws into the back
/// buffer (displayed, never sampled), keeps the draws into a render-to-texture surface first drawn in that frame, and the
/// next shown frame equals the unpruned reference.
#[test]
fn suppressed_present_prunes_unread_back_buffer_draws_only() {
    let clear = "call PushMethod 41d90 ff000000\ncall PushMethod 41d94 f0\n";
    let tri = triangle(
        0xFF00_FF00,
        [(100.0, 100.0), (400.0, 100.0), (100.0, 400.0)],
        [(0.0, 0.0); 3],
    );
    let back = format!(
        "{}{clear}mem 40002000 {tri}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n",
        set_surface(0x4100_0100, 0x1000, 1)
    );
    // Same frame plus a cleared draw into surface 0x2000, which only the shown frame samples.
    let with_rtt = format!(
        "{}{}call SetTexture 0 42000100\n{clear}call DrawVertices 5 0 3\n{back}",
        set_surface(0x4100_0000, 0x2000, 0),
        texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4]),
    );
    let quad = triangle(
        0xFF80_8080,
        [(450.0, 100.0), (550.0, 100.0), (450.0, 200.0)],
        [(100.0, 100.0), (200.0, 100.0), (100.0, 200.0)],
    );
    let record = hex(&[0x0004_0001, 0x2000, 0, (0x12 << 8) | 0x29, SIZE_640X480]);
    let shown = format!(
        "{}{clear}mem 40002100 {quad}\nhost 19240 40002100 18\nmem 42000000 {record}\ncall SetTexture 0 42000000\ncall DrawVertices 5 0 3\npresent\n",
        set_surface(0x4100_0100, 0x1000, 1)
    );
    let warm = format!(
        "{}call SetTexture 0 42000100\n{}{}",
        preamble(),
        texture_record(0x4200_0100, 0x1100, [0xFFFF_FFFF; 4]),
        with_rtt.repeat(9)
    );
    let run = |prune: bool| {
        let mut replayer = replay_with(&warm, |_| {})?;
        replayer.suppress_presentation = true;
        if !prune {
            replayer.knobs.insert("no_skip_unread".into(), "1".into());
        }
        replayer.run(&Stream::from_text(&with_rtt).unwrap());
        replayer.suppress_presentation = false;
        replayer.run(&Stream::from_text(&shown).unwrap());
        Some(replayer)
    };
    let (Some(pruned), Some(reference)) = (run(true), run(false)) else {
        return;
    };
    assert_eq!(reference.unread_draws_skipped, 0);
    assert_eq!(
        pruned.unread_draws_skipped, 1,
        "only the back buffer draw is pruned"
    );
    let frame = pruned.frames.last().unwrap();
    assert!(
        frame.pixel(470, 120)[1] > 225,
        "the render-to-texture draw was kept"
    );
    assert_eq!(frame.rgba, reference.frames.last().unwrap().rgba);
}
