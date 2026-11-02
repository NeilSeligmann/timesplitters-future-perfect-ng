// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 3: a FABRICATED stream that carries what the traced host now writes for a vertex program draw (the program
//! microcode at the `LoadVertexProgram` pointer, the 16 entry declaration block at the `SelectVertexDeclaration` pointer, vertex
//! bytes) and a texture record with swizzled texel data. No retail bytes. It checks the whole path: program upload, declaration
//! fetch, interpreter, window to clip conversion, texture decode and the modulate stand-in.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;

mod support;
use support::{declaration_hex, hex, vertex};

/// MOV out, v<input> (swizzle xyzw) to output register `address` with a full write mask.
fn mov_input_to_output(input: u32, address: u32, last: bool) -> [u32; 4] {
    let dword1 = (1 << 21) | (input << 9) | 0x1b;
    let dword2 = 2 << 26;
    let dword3 = (address << 3) | (1 << 11) | (0xF << 12) | u32::from(last);
    [0, dword1, dword2, dword3]
}

fn program_hex() -> String {
    // layout of d3d8_upload_vertex_program: u16 count at +2 (so the snapshot starts at program + 2), 16 bytes per instruction.
    let instructions = [
        mov_input_to_output(1, 0, false),
        mov_input_to_output(2, 3, false),
        mov_input_to_output(3, 9, true),
    ];
    let mut text = String::from("0300");
    for instruction in instructions {
        text.push_str(&hex(&instruction));
    }
    text
}

fn stream_text(with_texture: bool) -> String {
    let vertices = format!(
        "{}{}{}",
        vertex(100.0, 100.0, 0xFFFF_FFFF, 0.0, 0.0),
        vertex(400.0, 100.0, 0xFFFF_FFFF, 1.0, 0.0),
        vertex(100.0, 400.0, 0xFFFF_FFFF, 0.0, 1.0)
    );
    // 2x2 swizzled A8R8G8B8 texture (format code 6, log2 size 1 x 1): texels (x, y) in morton order are (0,0) (1,0) (0,1) (1,1)
    // for a 2x2, bytes B G R A. Red, green, blue, white.
    let texels = hex(&[0xFFFF_0000, 0xFF00_FF00, 0xFF00_00FF, 0xFFFF_FFFF]);
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    let record = hex(&[0x0004_0001, 0x8000_1000, 0, format_word, 0]);
    let texture_lines = if with_texture {
        format!("mem 42000000 {record}\nmem 80001000 {texels}\ncall SetTexture 0 42000000\n")
    } else {
        String::new()
    };
    format!(
        "# FABRICATED vertex program stream\n\
         mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         host 19240 40002000 18\n\
         mem 40002000 {vertices}\n\
         {texture_lines}\
         call DrawVertices 5 0 3\n\
         # present 1\n\
         present\n",
        program_hex(),
        declaration_hex()
    )
}

fn replay(text: &str) -> Option<Replayer> {
    let gpu = match GpuContext::new(&Preference::Software) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no software adapter): {error}");
            return None;
        }
    };
    let stream = Stream::from_text(text).expect("stream parses");
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(&stream);
    Some(replayer)
}

#[test]
fn vertex_program_draw_is_shaded_on_the_cpu() {
    let Some(replayer) = replay(&stream_text(false)) else {
        return;
    };
    assert_eq!(replayer.report.vsh_draws, 1);
    assert_eq!(replayer.report.vsh_vertices, 3);
    let frame = &replayer.frames[0];
    // The program copies the pixel position to oPos (w is the input default 1) and the D3DCOLOR to oD0: white triangle.
    let inside = frame.pixel(150, 150);
    assert_eq!(&inside[..3], &[255, 255, 255], "interior {inside:?}");
    let outside = frame.pixel(500, 450);
    assert_eq!(&outside[..3], &[0, 0, 0], "outside {outside:?}");
}

#[test]
fn swizzled_texture_modulates_the_diffuse_colour() {
    let Some(replayer) = replay(&stream_text(true)) else {
        return;
    };
    assert_eq!(replayer.report.texture_formats.get(&6), Some(&1));
    let frame = &replayer.frames[0];
    // u = (x - 100) / 300, v = (y - 100) / 300. Linear filtering on a 2x2 texture blends, so check the dominant channels near
    // the corners: top left texel red, top right (u -> 1) green is outside the triangle, bottom left (v -> 1) blue.
    let near_red = frame.pixel(110, 110);
    assert!(
        near_red[0] > 200 && near_red[1] < 90 && near_red[2] < 90,
        "{near_red:?}"
    );
    let near_blue = frame.pixel(108, 385);
    assert!(near_blue[2] > 150 && near_blue[0] < 120, "{near_blue:?}");
}

/// The engine sets up separate NV2A vertex arrays itself (MEASURED for the in level meshes): `VERTEX_DATA_ARRAY_OFFSET` (0x1720 +
/// 4i) and `_FORMAT` (0x1760 + 4i, stride in bits 8..31, size 4..7, type 0..3) per attribute register, no `BindVertexStream`.
fn arrays_stream_text() -> String {
    let positions = hex(&[
        100.0f32.to_bits(),
        100.0f32.to_bits(),
        0.5f32.to_bits(),
        400.0f32.to_bits(),
        100.0f32.to_bits(),
        0.5f32.to_bits(),
        100.0f32.to_bits(),
        400.0f32.to_bits(),
        0.5f32.to_bits(),
    ]);
    let colours = hex(&[0xFF00_FF00, 0xFF00_FF00, 0xFF00_FF00]);
    let uvs = hex(&[0, 0, 0, 0, 0, 0]);
    format!(
        "# FABRICATED separate vertex arrays\n\
         mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         mem 40002000 {positions}\n\
         mem 40003000 {colours}\n\
         mem 40004000 {uvs}\n\
         call PushMethod 41724 40002000\n\
         call PushMethod 41728 40003000\n\
         call PushMethod 4172c 40004000\n\
         call PushMethod 41764 c32\n\
         call PushMethod 41768 440\n\
         call PushMethod 4176c 822\n\
         call DrawVertices 5 0 3\n\
         present\n",
        program_hex(),
        declaration_hex()
    )
}

#[test]
fn separate_vertex_arrays_feed_the_program_inputs() {
    let Some(replayer) = replay(&arrays_stream_text()) else {
        return;
    };
    assert_eq!(replayer.report.vsh_draws, 1);
    let frame = &replayer.frames[0];
    // D3DCOLOR 0xFF00FF00 is opaque green.
    let inside = frame.pixel(150, 150);
    assert_eq!(&inside[..3], &[0, 255, 0], "interior {inside:?}");
    assert_eq!(&frame.pixel(500, 450)[..3], &[0, 0, 0]);
}

/// The pixel shader definition of the observed untextured shaders (colour = diffuse x 1 times 2, MEASURED word shapes, values
/// reproduced by hand), FABRICATED pointer: the combiner path of the GPU fragment stage must match `PixelProgram::evaluate`.
fn combiner_stream_text(final_abcd: u32, with_texture: bool) -> String {
    let mut words = vec![0u32; 0x40];
    words[0x88 / 4] = if with_texture {
        0xC4C8_0000
    } else {
        0xC420_0000
    };
    words[0xB4 / 4] = 0x0001_00C0;
    words[0x00] = if with_texture {
        0xD4D8_1010
    } else {
        0xD430_1010
    };
    words[0x68 / 4] = 0x0001_00C0;
    words[0x20 / 4] = final_abcd;
    words[0x24 / 4] = 0x0000_1C80;
    words[0xD4 / 4] = 0x0001_1101;
    words[0xD8 / 4] = u32::from(with_texture);
    let vertices = format!(
        "{}{}{}",
        vertex(100.0, 100.0, 0x8080_4040, 0.0, 0.0),
        vertex(400.0, 100.0, 0x8080_4040, 1.0, 0.0),
        vertex(100.0, 400.0, 0x8080_4040, 0.0, 1.0)
    );
    let texels = hex(&[0xFF80_8080; 4]); // 2x2 grey (0.502) opaque
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    let record = hex(&[0x0004_0001, 0x8000_1000, 0, format_word, 0]);
    let texture_lines = if with_texture {
        format!("mem 42000000 {record}\nmem 80001000 {texels}\ncall SetTexture 0 42000000\n")
    } else {
        String::new()
    };
    format!(
        "mem 40000002 {}\n\
         call LoadVertexProgram 40000000 0\n\
         mem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\n\
         host 19240 40002000 18\n\
         mem 40002000 {vertices}\n\
         mem 40003000 {}\n\
         call SetPixelShader 40003000\n\
         {texture_lines}\
         call DrawVertices 5 0 3\n\
         present\n",
        program_hex(),
        declaration_hex(),
        hex(&words)
    )
}

#[test]
fn combiner_scales_the_diffuse_colour_by_two() {
    // diffuse 0x80 0x40 0x40 -> (0.502, 0.251, 0.251) x 2 = (1.0, 0.5, 0.5); the alpha 0.502 x 2 clamps to 1 (opaque)
    let Some(replayer) = replay(&combiner_stream_text(0x0000_000C, false)) else {
        return;
    };
    let inside = replayer.frames[0].pixel(150, 150);
    assert!(
        inside[0] >= 254 && (126..=130).contains(&inside[1]) && (126..=130).contains(&inside[2]),
        "interior {inside:?}"
    );
}

#[test]
fn combiner_c_form_final_and_texture_modulation() {
    // final combiner (1 - 0) * spare 0 (the C form the observed shaders also use), colour = diffuse x texture x 2:
    // (0.502, 0.251, 0.251) x 0.502 x 2 = (0.504, 0.252, 0.252) -> about (129, 64, 64)
    let Some(replayer) = replay(&combiner_stream_text(0x0000_0C00, true)) else {
        return;
    };
    let inside = replayer.frames[0].pixel(150, 150);
    assert!(
        (126..=132).contains(&inside[0])
            && (61..=67).contains(&inside[1])
            && (61..=67).contains(&inside[2]),
        "interior {inside:?}"
    );
}
