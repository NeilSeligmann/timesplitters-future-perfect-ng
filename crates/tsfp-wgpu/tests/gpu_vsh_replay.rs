// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024k: the GPU vertex program path inside `Replayer` on a FABRICATED stream (no retail bytes): a pass through program (inputs
//! 1, 2, 3 to the position, colour and texture coordinate outputs) draws a 16 x 12 grid of coloured triangles (576 distinct
//! vertices, above the GPU path's threshold). The frame rendered with `set_gpu_vsh(Some(true))` after the program's pipeline is ready
//! must be pixel identical to the CPU interpreter frame, and the counters must prove the GPU path ran (not a vacuous equality).
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;

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

fn frame() -> String {
    frame_with_specular(false)
}

fn frame_with_specular(specular: bool) -> String {
    let mut vertices = String::new();
    let mut count = 0;
    for row in 0..12 {
        for column in 0..16 {
            let (x, y) = (column as f32 * 40.0, row as f32 * 40.0);
            let colour = 0xFF00_0000 | ((column * 16) << 16) | ((row * 20) << 8) | 0x40;
            for (dx, dy) in [(0.0, 0.0), (36.0, 0.0), (0.0, 36.0)] {
                vertices.push_str(&vertex(x + dx, y + dy, colour, dx, dy));
                count += 1;
            }
        }
    }
    let mut program = program_hex();
    let mut pixel_setup = String::new();
    if specular {
        // Position and oD1 only: diffuse remains black. The combiner must read V1.
        let instructions = [
            mov_input_to_output(1, 0, false),
            mov_input_to_output(2, 4, true),
        ];
        program = format!("{:02x}00", instructions.len());
        for instruction in instructions {
            program.push_str(&hex(&instruction));
        }
        let mut words = vec![0u32; 0x40];
        words[0xD4 / 4] = 1; // one unused stage; zero count disables pixel programs
        words[0x20 / 4] = 5; // final D = specular RGB
        words[0x24 / 4] = 0x1500; // final G = specular alpha
        pixel_setup = format!(
            "mem 40003000 {}\ncall SetPixelShader 40003000\n",
            hex(&words)
        );
    }
    format!(
        "mem 40000002 {program}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {declaration}\ncall SelectVertexDeclaration 40001000 0\n\
         {pixel_setup}mem 40002000 {vertices}\nhost 19240 40002000 18\ncall SetVertexConstants 0 0 0 0 0\ncall DrawVertices 5 0 {count:x}\ncall SetVertexConstants 0 80000000 0 0 0\ncall DrawVertices 5 0 {count:x}\ncall SetVertexConstants 0 0 0 0 0\ncall DrawVertices 5 0 {count:x}\npresent\n",
        program = program,
        declaration = declaration_hex(),
    )
}

fn replayer(on: bool) -> Option<Replayer> {
    let gpu = match GpuContext::new(&Preference::Any) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no adapter): {error}");
            return None;
        }
    };
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.set_gpu_vsh(Some(on));
    if on {
        replayer.knobs.insert("batch_uploads".into(), "1".into());
    }
    Some(replayer)
}

#[test]
fn the_gpu_vertex_path_draws_the_same_frame_as_the_interpreter() {
    let stream = Stream::from_text(&frame()).expect("stream parses");
    let (Some(mut off), Some(mut on)) = (replayer(false), replayer(true)) else {
        return;
    };
    off.run(&stream);
    assert!(
        !on.prewarm_vertex_program(),
        "no program has been selected yet"
    );
    let setup = frame()
        .split("call DrawVertices")
        .next()
        .unwrap()
        .to_string();
    on.run(&Stream::from_text(&setup).unwrap());
    assert!(on.prewarm_vertex_program());
    assert!(on.prewarm_vertex_program());
    assert_eq!(
        on.gpu_vsh_stats().programs_requested,
        1,
        "prewarm requests deduplicate"
    );
    assert_eq!(on.gpu_vsh_stats().gpu_draws, 0);
    assert_eq!(on.gpu_vsh_stats().upload_batches, 0);
    assert!(on.frames.is_empty(), "prewarm must not present");
    // the first frames run on the CPU while the worker builds the compute pipeline
    for _ in 0..200 {
        on.run(&stream);
        if on.gpu_vsh_stats().gpu_draws > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    // Compilation can finish between draws of the first GPU frame. Repeat the
    // full frame after readiness before requiring within-frame cache hits.
    on.run(&stream);
    let stats = on.gpu_vsh_stats();
    if stats.programs_failed > 0 {
        println!("SKIPPED (this adapter rejected the compute shader)");
        return;
    }
    assert!(
        stats.input_reuses > 0,
        "repeated attributes were not reused: {stats:?}"
    );
    assert!(
        stats.constant_reuses > 0,
        "repeated constants were not reused: {stats:?}"
    );
    assert!(stats.upload_batches > 0);
    assert!(stats.gpu_draws > 0, "the GPU path never ran: {stats:?}");
    let before = on.gpu_vsh_stats();
    on.run(&stream);
    let after = on.gpu_vsh_stats();
    assert_eq!(after.input_reuses - before.input_reuses, 2);
    assert_eq!(
        after.constant_reuses - before.constant_reuses,
        1,
        "A/B/A constant reuse"
    );
    assert_eq!(
        after.constant_bytes - before.constant_bytes,
        2 * 192 * 4 * 4,
        "signed-zero snapshots must remain distinct"
    );
    let gpu_draws = on.gpu_vsh_stats().gpu_draws;
    on.knobs
        .insert("gpu_vsh_min_vertices".into(), "10000".into());
    on.run(&stream);
    assert_eq!(
        on.gpu_vsh_stats().gpu_draws,
        gpu_draws,
        "high threshold must route to CPU"
    );
    assert!(on.gpu_vsh_stats().small_draws > 0);
    on.knobs.insert("gpu_vsh_min_vertices".into(), "24".into());
    on.run(&stream);
    let expected = off.frames.last().expect("a frame");
    let got = on.frames.last().expect("a frame");
    assert_eq!(
        got.rgba,
        expected.rgba,
        "GPU and CPU vertex shading differ ({stats:?}, {})",
        on.adapter_name()
    );
    // not an empty frame: the grid covers pixels
    let lit = expected
        .rgba
        .chunks_exact(4)
        .filter(|px| px[0] | px[1] | px[2] != 0)
        .count();
    assert!(lit > 20_000, "only {lit} lit pixels");
    assert_eq!(
        off.gpu_vsh_stats().gpu_draws,
        0,
        "the off replayer used the GPU path"
    );
}

#[test]
fn specular_only_mesh_is_lit_in_cpu_and_gpu_paths() {
    let text = frame_with_specular(true);
    let stream = Stream::from_text(&text).unwrap();
    let (Some(mut cpu), Some(mut gpu)) = (replayer(false), replayer(true)) else {
        return;
    };
    cpu.run(&stream);
    let expected = cpu.frames.last().unwrap();
    let pixel = expected.pixel(45, 45);
    assert!(
        pixel[0] > 0 && pixel[1] > 0 && pixel[2] > 0,
        "oD1 lost: {pixel:?}"
    );
    for _ in 0..200 {
        gpu.run(&stream);
        if gpu.gpu_vsh_stats().gpu_draws > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(
        gpu.gpu_vsh_stats().gpu_draws > 0,
        "GPU path must actually run"
    );
    gpu.run(&stream);
    assert_eq!(gpu.frames.last().unwrap().rgba, expected.rgba);
}

#[test]
fn raw_library_constants_light_meshes_without_a_shadow_snapshot() {
    let mut instructions = vec![mov_input_to_output(1, 0, false)];
    // MOV oD0, c[10]. All lighting comes from the actual upload, not a trace shadow.
    instructions.push([
        0,
        (1 << 21) | (10 << 13) | 0x1b,
        3 << 26,
        (3 << 3) | (1 << 11) | (0xF << 12) | 1,
    ]);
    let mut program = format!("{:02x}00", instructions.len());
    for instruction in instructions {
        program.push_str(&hex(&instruction));
    }
    let text = frame().replace(&program_hex(), &program);
    let upload = "call SetVertexConstants a 3e800000 3f000000 3f400000 3f800000\n";
    let expected_text = text.replace(
        "call DrawVertices",
        &(upload.to_string() + "call DrawVertices"),
    );
    // Methods need not repeat B80: the cursor advances for every aligned data method.
    let raw =
        "lib 1ea4 a\nlib b80 3e800000\nlib b84 3f000000\nlib b88 3f400000\nlib b8c 3f800000\n";
    let raw_text = text.replace(
        "call DrawVertices",
        &(raw.to_string() + "call DrawVertices"),
    );
    let (Some(mut expected), Some(mut cpu), Some(mut gpu)) =
        (replayer(false), replayer(false), replayer(true))
    else {
        return;
    };
    expected.run(&Stream::from_text(&expected_text).unwrap());
    cpu.run(&Stream::from_text(&raw_text).unwrap());
    let frame = expected.frames.last().unwrap();
    assert!(frame.pixel(45, 45)[0] > 0);
    assert_eq!(cpu.frames.last().unwrap().rgba, frame.rgba);
    let stream = Stream::from_text(&raw_text).unwrap();
    for _ in 0..200 {
        gpu.run(&stream);
        if gpu.gpu_vsh_stats().gpu_draws > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(gpu.gpu_vsh_stats().gpu_draws > 0);
    gpu.run(&stream);
    assert_eq!(gpu.frames.last().unwrap().rgba, frame.rgba);
}
