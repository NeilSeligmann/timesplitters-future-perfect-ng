// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 6: a vertex constant written before the recording window (the constant file line a skip stream starts with) is
//! what lights a model, and the host raster map of `vsh::to_clip_host_raster`. FABRICATED streams, no retail bytes.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;
use tsfp_wgpu::vsh;

mod support;
use support::hex;

fn mov_input_to_output(input: u32, address: u32, last: bool) -> [u32; 4] {
    let dword1 = (1 << 21) | (input << 9) | 0x1b;
    let dword2 = 2 << 26;
    let dword3 = (address << 3) | (1 << 11) | (0xF << 12) | u32::from(last);
    [0, dword1, dword2, dword3]
}

/// `MUL o<address>, v<input>, c[constant].zzzz`, the diffuse scale the lit world programs of the retail game end with (c10.z).
fn mul_input_by_constant_z(input: u32, constant: u32, address: u32, last: bool) -> [u32; 4] {
    let dword1 = (2 << 21) | (constant << 13) | (input << 9) | 0x1b;
    let dword2 = (2 << 26) | (3 << 11) | (0xAA << 17);
    let dword3 = (address << 3) | (1 << 11) | (0xF << 12) | u32::from(last);
    [0, dword1, dword2, dword3]
}

fn declaration_hex() -> String {
    let mut words = vec![0u32; 64];
    for entry in 0..16 {
        words[entry * 4 + 2] = 2;
    }
    words[4 + 2] = 0x32; // v1 float3 at 0
    words[8 + 1] = 12;
    words[8 + 2] = 0x40; // v2 D3DCOLOR at 12
    words[12 + 1] = 16;
    words[12 + 2] = 0x22;
    hex(&words)
}

fn program_hex() -> String {
    let instructions = [
        mov_input_to_output(1, 0, false),
        mul_input_by_constant_z(2, 10, 3, true),
    ];
    let mut text = format!("{:02x}00", instructions.len());
    for instruction in instructions {
        text.push_str(&hex(&instruction));
    }
    text
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

fn vertex(x: f32, y: f32) -> String {
    vertex_with_u(x, y, 0.0)
}

fn vertex_with_u(x: f32, y: f32, u: f32) -> String {
    hex(&[
        x.to_bits(),
        y.to_bits(),
        0.5f32.to_bits(),
        0xFFFF_FFFF,
        u.to_bits(),
        0,
    ])
}

/// A white triangle whose program writes `oFog` from the texture coordinate u (0.5) and whose final combiner is the fog form
/// `fog.a * fog.rgb + (1 - fog.a) * spare 0`. `library` is what the stream carries for the library's fog state.
fn fog_stream(library: &str) -> String {
    let instructions = [
        mov_input_to_output(1, 0, false),
        mov_input_to_output(2, 3, false),
        mov_input_to_output(3, 5, true),
    ];
    let mut program = format!("{:02x}00", instructions.len());
    for instruction in instructions {
        program.push_str(&hex(&instruction));
    }
    let mut shader = pixel_shader();
    shader[0x20 / 4] = 0x1303_0C00;
    let triangle = format!(
        "{}{}{}",
        vertex_with_u(100.0, 100.0, 0.5),
        vertex_with_u(400.0, 100.0, 0.5),
        vertex_with_u(100.0, 400.0, 0.5)
    );
    format!(
        "{library}mem 40000002 {program}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\nmem 40003000 {}\ncall SetPixelShader 40003000\n\
         mem 40002000 {triangle}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n",
        declaration_hex(),
        hex(&shader)
    )
}

/// A white triangle lit through `c10.z`. `constant_line` is what the stream carries for the constant file.
fn stream(constant_line: &str) -> String {
    let triangle = format!(
        "{}{}{}",
        vertex(100.0, 100.0),
        vertex(400.0, 100.0),
        vertex(100.0, 400.0)
    );
    format!(
        "{constant_line}mem 40000002 {}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {}\n\
         call SelectVertexDeclaration 40001000 0\nmem 40003000 {}\ncall SetPixelShader 40003000\n\
         mem 40002000 {triangle}\nhost 19240 40002000 18\ncall DrawVertices 5 0 3\npresent\n",
        program_hex(),
        declaration_hex(),
        hex(&pixel_shader())
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
fn a_constant_written_before_the_window_lights_the_draw() {
    // c10 = (0, 1, 0.5, 0) as in the constant shadow of a retail in level frame: only the z component scales the diffuse.
    let lit_line = format!(
        "call SetVertexConstants a {}\n",
        [0.0f32, 1.0, 0.5, 0.0]
            .iter()
            .map(|value| format!("{:x}", value.to_bits()))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let Some(lit) = replay(&stream(&lit_line)) else {
        return;
    };
    let pixel = lit.frames[0].pixel(150, 150);
    assert!(
        pixel[0] > 200 && pixel[1] > 200 && pixel[2] > 200,
        "constant file line present: {pixel:?}"
    );
    // Control (the slice 5 skip stream): without the line c10 is 0, the diffuse is 0 and the model is black.
    let Some(black) = replay(&stream("")) else {
        return;
    };
    let pixel = black.frames[0].pixel(150, 150);
    assert!(
        pixel[0] < 10 && pixel[1] < 10 && pixel[2] < 10,
        "no constant file line: {pixel:?}"
    );
}

#[test]
fn host_raster_map_clamps_w_snaps_to_sixteenths_and_keeps_the_depth_unscaled() {
    let clip = vsh::to_clip_host_raster(
        [320.03, 240.0, 8_388_607.5, 2.0],
        (640.0, 480.0),
        16_777_215.0,
    )
    .expect("finite");
    // x snaps to 320.0, so the centre of the screen
    assert!(clip[0].abs() < 1e-6 && clip[1].abs() < 1e-6, "{clip:?}");
    assert!((clip[2] - 1.0).abs() < 1e-6 && clip[3] == 2.0, "{clip:?}");
    // a negative w keeps its sign and a tiny one is clamped to 2^-64
    let behind = vsh::to_clip_host_raster([0.0, 0.0, 0.0, -1e-30], (640.0, 480.0), 16_777_215.0)
        .expect("finite");
    assert_eq!(behind[3], -f32::from_bits(0x1F80_0000));
    assert!(vsh::to_clip_host_raster([f32::NAN, 0.0, 0.0, 1.0], (640.0, 480.0), 1.0).is_none());
}

#[test]
fn library_fog_state_blends_the_fog_colour_by_the_vertex_fog_amount() {
    // linear mode 0x2601 with parameters (1, 1): the amount is the distance itself, here 0.5, towards a pure red fog colour
    let library = "lib 29c 2601\nlib 9c0 3f800000\nlib 9c4 3f800000\nlib 2a8 ff\nlib 2a4 1\n";
    let Some(fogged) = replay(&fog_stream(library)) else {
        return;
    };
    let pixel = fogged.frames[0].pixel(150, 150);
    assert!(
        pixel[0] > 245 && (118..=138).contains(&pixel[1]) && (118..=138).contains(&pixel[2]),
        "half fogged white: {pixel:?}"
    );
    // fog switched off (enable 0) or never pushed: spare 0 passes through, the draw stays white
    for off in ["lib 2a4 0\n", ""] {
        let Some(plain) = replay(&fog_stream(off)) else {
            return;
        };
        let pixel = plain.frames[0].pixel(150, 150);
        assert!(
            pixel[1] > 245 && pixel[2] > 245,
            "no fog {off:?}: {pixel:?}"
        );
    }
}

#[test]
fn describe_lists_the_instructions_of_a_program() {
    let mut memory = vsh::ProgramMemory::default();
    memory.slots[0] = mov_input_to_output(1, 0, false);
    memory.slots[1] = mul_input_by_constant_z(2, 10, 3, true);
    let text = vsh::describe(&memory, 0);
    assert!(
        text.contains("c[10]") && text.contains("LAST") && text.contains("o3"),
        "{text}"
    );
}
