// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic shader and texture builders for the slice 4/5 contract tests.

use super::support::hex;

/// MOV out, v<input> (swizzle xyzw) to output register `address` with a full write mask.
fn mov_input_to_output(input: u32, address: u32, last: bool) -> [u32; 4] {
    let dword1 = (1 << 21) | (input << 9) | 0x1b;
    let dword2 = 2 << 26;
    let dword3 = (address << 3) | (1 << 11) | (0xF << 12) | u32::from(last);
    [0, dword1, dword2, dword3]
}

/// Program: position, diffuse, texture coordinate 0, and with `two_coordinates` coordinate 1 too (both from input 3).
pub fn program_hex(two_coordinates: bool) -> String {
    let mut instructions = vec![
        mov_input_to_output(1, 0, false),
        mov_input_to_output(2, 3, false),
        mov_input_to_output(3, 9, !two_coordinates),
    ];
    if two_coordinates {
        instructions.push(mov_input_to_output(3, 10, true));
    }
    let mut text = format!("{:02x}00", instructions.len());
    for instruction in instructions {
        text.push_str(&hex(&instruction));
    }
    text
}

/// A 2x2 swizzled A8R8G8B8 texture record at `address` whose texels (morton order) are `texels`.
pub fn texture_record(address: u32, data: u32, texels: [u32; 4]) -> String {
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    format!(
        "mem {address:x} {}\nmem {:x} {}\n",
        hex(&[0x0004_0001, 0x8000_0000 | data, 0, format_word, 0]),
        0x8000_0000 | data,
        hex(&texels)
    )
}

/// Pixel shader definition words: stage 0 colour `rgb_inputs`, alpha diffuse x texture or diffuse, final C form, `modes`.
pub fn pixel_shader(rgb_inputs: u32, alpha_inputs: u32, modes: u32) -> Vec<u32> {
    let mut words = vec![0u32; 0x40];
    words[0x88 / 4] = rgb_inputs;
    words[0xB4 / 4] = 0x0001_00C0; // AB to spare 0, times 2
    words[0x00] = alpha_inputs;
    words[0x68 / 4] = 0x0001_00C0;
    words[0x20 / 4] = 0x0000_0C00; // final: (1 - 0) x spare 0
    words[0x24 / 4] = 0x0000_1C80;
    words[0xD4 / 4] = 0x0001_1101;
    words[0xD8 / 4] = modes;
    words
}
