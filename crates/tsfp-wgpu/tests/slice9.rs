// SPDX-License-Identifier: GPL-3.0-or-later
//! T2140 (slice 9): the closing grade pass and the combiner constants the library pushes after `SetPixelShader`, on FABRICATED
//! streams (no retail bytes). The pass shape is the one the engine ends every frame with: a quad strip into the other buffer of the
//! flip chain, One/Zero, no depth, a combiner that samples texture 0 while no texture is bound (`SetTexture 0 0`).

#[path = "support/replay.rs"]
mod replay_support;
use replay_support::replay_with;

mod support;

/// T2336: the display regrade test switches the process-wide display-rate mode; the tests of this file run one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}
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

/// Combiner input byte: mapping (bits 7..5), channel (bit 4: alpha) and register (bits 3..0).
fn input(mapping: u32, alpha: bool, register: u32) -> u32 {
    (mapping << 5) | (u32::from(alpha) << 4) | register
}

fn word(a: u32, b: u32, c: u32, d: u32) -> u32 {
    (a << 24) | (b << 16) | (c << 8) | d
}

/// Texture times diffuse, doubled, into spare 0 (the engine's usual textured stage).
fn modulate_shader() -> Vec<u32> {
    let mut words = vec![0u32; 0x40];
    words[0x88 / 4] = word(input(6, false, 4), input(6, false, 8), 0, 0);
    words[0xB4 / 4] = 0x0001_00C0;
    words[0x00] = word(
        input(6, true, 4),
        input(6, true, 8),
        input(0, true, 0),
        input(0, true, 0),
    );
    words[0x68 / 4] = 0x0001_00C0;
    words[0x20 / 4] = 0x0000_0C00;
    words[0x24 / 4] = 0x0000_1C80;
    words[0xD4 / 4] = 0x0001_1101;
    words[0xD8 / 4] = 1;
    words
}

/// The grade: luma (product with the stage 0 constant 0, dot mode), saturation boost by the stage 2 constant 0, gain and bias by the
/// stage 3 constants, five stages, unique constants per stage.
fn grade_shader() -> Vec<u32> {
    let one = input(1, false, 0); // inverted zero
    let mut words = vec![0u32; 0x40];
    words[0x28 / 4] = 0x004D_961C; // stage 0 constant 0: luma weights
    let colour_inputs = [
        word(input(6, false, 8), input(6, false, 1), 0, 0),
        word(input(6, false, 8), one, input(7, false, 0xD), one),
        // T2347: the retail definition reads the saturation from the alpha of the stage 2 constant 0
        word(
            input(6, false, 0xC),
            input(6, true, 1),
            one,
            input(6, false, 8),
        ),
        word(
            input(6, false, 0xD),
            input(6, false, 1),
            one,
            input(7, false, 2),
        ),
        word(input(6, false, 0xC), one, input(6, false, 0xD), one),
    ];
    words[0x88 / 4..0x88 / 4 + 5].copy_from_slice(&colour_inputs);
    let colour_outputs = [0x0008_20D0, 0xC00, 0xD00, 0xC00, 0xC00];
    words[0xB4 / 4..0xB4 / 4 + 5].copy_from_slice(&colour_outputs);
    words[0x20 / 4] = 0x0000_0C00;
    words[0x24 / 4] = 0x0000_1C80;
    words[0xD4 / 4] = 0x0001_1105;
    words[0xD8 / 4] = 1;
    words
}

fn size_word(width: u32, height: u32) -> u32 {
    (width - 1) | ((height - 1) << 12) | (39 << 24)
}

fn set_surface(header: u32, data: u32) -> String {
    format!(
        "call SetSurface {header:x} 0 {data:x} 11229 {:x} {:x} 0 0\n",
        size_word(640, 480),
        640 * 4
    )
}

fn white_texture() -> String {
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    format!(
        "mem 42000100 {}\nmem 80001100 {}\n",
        hex(&[0x0004_0001, 0x8000_1100, 0, format_word, 0]),
        hex(&[0xFFFF_FFFF; 4])
    )
}

/// One frame (it ends on the scene target again, like the engine's, so the next frame starts there): grey scene triangle into surface A, then the closing pass into surface B. `constants` pushes the library's stage
/// constants after the pass program was bound (the real engine sets them with `SetPixelShaderConstant`).
fn frame(constants: bool) -> String {
    let once = frame_once(constants);
    format!("{once}{once}")
}

fn frame_once(constants: bool) -> String {
    frame_coloured(constants, [0xFF64_6464; 3])
}

/// One frame whose scene triangle has the vertex colours `colours` (a gradient exercises every grade stage).
fn frame_coloured(constants: bool, colours: [u32; 3]) -> String {
    let scene = [(0.0, 0.0), (640.0, 0.0), (0.0, 480.0)]
        .iter()
        .zip(colours)
        .map(|(&(x, y), colour)| vertex(x, y, colour, 0.0, 0.0))
        .collect::<String>();
    let quad = [(0.0, 0.0), (640.0, 0.0), (0.0, 480.0), (640.0, 480.0)]
        .iter()
        .map(|&(x, y)| vertex(x, y, 0, x, y))
        .collect::<String>();
    let pushes = if constants {
        "lib a68 1f1f1f1f\nlib a6c 17171717\nlib a8c 0a0a0a0a\n"
    } else {
        ""
    };
    // blend enable, source One, destination Zero (the engine's closing pass), then the quad strip
    let blend = "call PushMethod 40304 1\ncall PushMethod 40344 1\ncall PushMethod 40348 0\ncall PushMethod 4030c 0\n";
    format!(
        "mem 40000002 {program}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {declaration}\ncall SelectVertexDeclaration 40001000 0\n\
         mem 40003000 {modulate}\n{white}{surface_a}call SetPixelShader 40003000\ncall SetTexture 0 42000100\nmem 40002000 {scene}\n\
         host 19240 40002000 18\ncall DrawVertices 5 0 3\n\
         {surface_b}call SetTexture 0 0\nmem 40004000 {grade}\ncall SetPixelShader 40004000\n{pushes}{blend}\
         mem 40002100 {quad}\nhost 19240 40002100 18\ncall DrawVertices 6 0 4\n{surface_a}present\n",
        program = program_hex(),
        declaration = declaration_hex(),
        modulate = hex(&modulate_shader()),
        grade = hex(&grade_shader()),
        white = white_texture(),
        surface_a = set_surface(0x4100_0000, 0x1000),
        surface_b = set_surface(0x4100_0100, 0x2000),
    )
}

fn with_vertex_bindings(source: &str, typed: bool) -> String {
    let mut output = String::new();
    for line in source.lines() {
        output.push_str(line);
        output.push('\n');
        let Some(address) = line.strip_prefix("host 19240 ") else {
            continue;
        };
        let base = u32::from_str_radix(address.split_whitespace().next().unwrap(), 16).unwrap();
        let mut formats = [0u32; 16];
        let mut offsets = [0u32; 16];
        for (index, relative, format) in [(1usize, 0u32, 0x32u32), (2, 12, 0x40), (3, 16, 0x22)] {
            formats[index] = (24 << 8) | format;
            offsets[index] = base + relative;
        }
        if typed {
            output.push_str("call SetVertexBindings");
            for word in formats.iter().chain(&offsets) {
                output.push_str(&format!(" {word:x}"));
            }
            output.push('\n');
        } else {
            for index in [1u32, 2, 3] {
                output.push_str(&format!(
                    "call PushMethod {:x} {:x}\ncall PushMethod {:x} {:x}\n",
                    0x1720 + 4 * index,
                    offsets[index as usize],
                    0x1760 + 4 * index,
                    formats[index as usize]
                ));
            }
        }
    }
    output
}

#[test]
fn typed_vertex_bindings_match_their_nv2a_method_pairs() {
    let _serial = serial();
    let source = frame(true);
    let typed_text = with_vertex_bindings(&source, true);
    let methods_text = with_vertex_bindings(&source, false);
    let Some(typed) = replay_with(&typed_text, |_| {}) else {
        return;
    };
    let Some(methods) = replay_with(&methods_text, |_| {}) else {
        return;
    };
    assert_eq!(typed.frames.len(), methods.frames.len());
    for (index, (typed, methods)) in typed.frames.iter().zip(&methods.frames).enumerate() {
        assert_eq!(
            typed.hash(),
            methods.hash(),
            "typed bindings differ from 0x1720/0x1760 method writes in frame {index}"
        );
    }
}

#[test]
fn empty_and_inverted_scissors_do_not_expand_to_the_full_viewport() {
    let _serial = serial();
    let source = frame(false);
    let Some(full_viewport) = replay_with(&source, |_| {}) else {
        return;
    };
    for (name, rect) in [
        ("zero-width", "16 16 16 32"),
        ("inverted horizontal", "20 16 12 32"),
        ("inverted vertical", "16 20 32 12"),
    ] {
        let Some(clipped) = replay_with(&format!("call SetScissor 1 0 {rect}\n{source}"), |_| {})
        else {
            return;
        };

        assert_eq!(full_viewport.frames.len(), clipped.frames.len());
        assert!(
            full_viewport
                .frames
                .iter()
                .zip(&clipped.frames)
                .any(|(full, clipped)| full.hash() != clipped.hash()),
            "a {name} scissor must not be treated as the full viewport"
        );
    }
}

#[test]
fn the_closing_pass_grades_the_scene_target_into_the_displayed_image() {
    let _serial = serial();
    let Some(graded) = replay_with(&frame(true), |_| {}) else {
        return;
    };
    let shown = graded.frames[1].pixel(100, 100);
    // scene grey 100 doubled is 200, the grade is gain 1.09 and bias -10: 208 (saturation does not move a grey)
    for channel in 0..3 {
        assert!(
            (206..=210).contains(&shown[channel]),
            "channel {channel} of the graded grey, got {shown:?}"
        );
    }
}

#[test]
fn the_grade_needs_both_the_pass_and_the_library_constants() {
    let _serial = serial();
    let Some(no_pass) = replay_with(&frame(true), |r| {
        r.knobs.insert("no_grade_pass".into(), "1".into());
    }) else {
        return;
    };
    let grey = no_pass.frames[1].pixel(100, 100);
    assert!(
        (198..=202).contains(&grey[0]),
        "control, no pass: the scene target is shown, got {grey:?}"
    );
    let Some(no_constants) = replay_with(&frame(false), |_| {}) else {
        return;
    };
    let identity = no_constants.frames[1].pixel(100, 100);
    assert!(
        (198..=202).contains(&identity[0]),
        "control, no constants pushed: the pass is the identity, got {identity:?}"
    );
    let Some(ignored) = replay_with(&frame(true), |r| {
        r.knobs.insert("no_lib_combiner".into(), "1".into());
    }) else {
        return;
    };
    let pixel = ignored.frames[1].pixel(100, 100);
    assert!(
        (198..=202).contains(&pixel[0]),
        "control, pushes ignored: the pass is the identity, got {pixel:?}"
    );
}

/// T2347: the native grade pass (typed `GradeModel`, own wgpu pass) is hash-exact against the replayed combiner draw.
#[test]
fn the_native_grade_is_hash_exact_against_the_replayed_grade() {
    let _serial = serial();
    let text = {
        let once = frame_coloured(true, [0xFFF0_2010, 0xFF10_C0F0, 0xFF80_8000]);
        format!("{once}{once}")
    };
    let Some(native) = replay_with(&text, |_| {}) else {
        return;
    };
    let Some(replayed) = replay_with(&text, |r| {
        r.knobs.insert("replay_grade".into(), "1".into());
    }) else {
        return;
    };
    assert!(
        native.native_grades >= 2 && native.replayed_grades == 0,
        "native {} replayed {}",
        native.native_grades,
        native.replayed_grades
    );
    assert!(replayed.native_grades == 0 && replayed.replayed_grades >= 2);
    assert!(native.last_grade.is_some() && native.last_grade == replayed.last_grade);
    let (a, b) = (&native.frames[1].rgba, &replayed.frames[1].rgba);
    assert!(!a.is_empty() && a.len() == b.len());
    let distinct: std::collections::HashSet<_> = a.chunks_exact(4).collect();
    assert!(
        distinct.len() > 1000,
        "gradient expected, {} colours",
        distinct.len()
    );
    let differing = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(x, y)| x != y)
        .count();
    assert_eq!(
        differing, 0,
        "pixels differing between the native and the replayed grade"
    );
}

/// T2336: the display regrade keeps the original order (clean pre-grade scene, view-pass draw, grade). Outside the
/// view-pass draw it is byte equal to the guest frame's grade; the view-pass draw itself comes out graded.
#[test]
fn the_display_regrade_grades_display_rate_view_pass_draws_in_the_original_order() {
    let _serial = serial();
    use tsfp_wgpu::frame_layer::{self, FrameLayer, LayerTarget};
    struct Reticle(wgpu::Texture);
    impl FrameLayer for Reticle {
        fn encode(&mut self, _target: LayerTarget<'_>) -> bool {
            false
        }
        fn encode_display_view_pass(&mut self, target: LayerTarget<'_>) -> bool {
            let mut at = target.target.as_image_copy();
            at.origin = wgpu::Origin3d {
                x: 100,
                y: 100,
                z: 0,
            };
            target
                .encoder
                .copy_texture_to_texture(self.0.as_image_copy(), at, self.0.size());
            true
        }
    }
    frame_layer::set_display_rate(true);
    let text = {
        let once = frame_coloured(true, [0xFFF0_2010, 0xFF10_C0F0, 0xFF80_8000]);
        format!("{once}{once}")
    };
    let replayed = replay_with(&text, |_| {});
    let Some(replayer) = replayed else {
        frame_layer::set_display_rate(false);
        return;
    };
    let (device, queue) = replayer.device_and_queue();
    let grade = frame_layer::display_grade(device, replayer.readback_present)
        .expect("native grade latched for the display");
    let size = grade.pregrade.size();
    let texture = |label, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let grey = texture(
        "grey",
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
    );
    queue.write_texture(
        grey.as_image_copy(),
        &[200u8, 200, 200, 255].repeat(256),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(64),
            rows_per_image: None,
        },
        grey.size(),
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("display work"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    frame_layer::install("t2336-test", 0, Box::new(Reticle(grey)));
    let mut encoder = device.create_command_encoder(&Default::default());
    let (before, _) = frame_layer::display_regrade_stats();
    frame_layer::encode_display(LayerTarget {
        device,
        queue,
        encoder: &mut encoder,
        target: &target,
        frame: 0,
        present: replayer.readback_present,
    });
    frame_layer::remove("t2336-test");
    frame_layer::set_display_rate(false);
    assert_eq!(
        frame_layer::display_regrade_stats().0,
        before + 1,
        "the display update regraded"
    );
    let row = (size.width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = buffer.slice(..).get_mapped_range();
    let shown = &replayer.frames[1];
    assert_eq!(
        (shown.width, size.height),
        (size.width, shown.rgba.len() as u32 / 4 / size.width)
    );
    let (x0, y0, w, h) = grade.scissor;
    let graded = grade
        .model
        .apply([200.0 / 255.0, 200.0 / 255.0, 200.0 / 255.0, 1.0]);
    let expected = &graded[..3]
        .iter()
        .map(|c| (c * 255.0).round() as u8)
        .collect::<Vec<_>>();
    assert!(
        expected.iter().all(|c| c.abs_diff(200) > 2),
        "the grade moves the grey, else the order is not observable"
    );
    let (mut equal, mut reticle) = (0, 0);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let at = (y * row + x * 4) as usize;
            let pixel = &data[at..at + 3];
            if (100..116).contains(&x) && (100..116).contains(&y) {
                assert!(
                    pixel.iter().zip(expected).all(|(c, e)| c.abs_diff(*e) <= 1),
                    "graded grey reticle, expected {expected:?}, got {pixel:?}"
                );
                reticle += 1;
            } else {
                assert_eq!(
                    pixel,
                    &shown.pixel(x, y)[..3],
                    "pixel {x} {y} differs from the guest frame grade"
                );
                equal += 1;
            }
        }
    }
    assert!(
        reticle == 256 && equal > 1000,
        "reticle {reticle} equal {equal}"
    );
}
