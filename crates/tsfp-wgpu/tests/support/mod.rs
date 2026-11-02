// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic stream builders shared by the renderer contract tests.

/// Serialize words in the trace's little-endian byte order.
pub fn hex(words: &[u32]) -> String {
    words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Encode the synthetic position, diffuse and UV declaration.
pub fn declaration_hex() -> String {
    let mut words = vec![0u32; 64];
    for entry in 0..16 {
        words[entry * 4 + 2] = 2;
    }
    words[4 + 1] = 0;
    words[4 + 2] = 0x32;
    words[8 + 1] = 12;
    words[8 + 2] = 0x40;
    words[12 + 1] = 16;
    words[12 + 2] = 0x22;
    hex(&words)
}

/// Encode one synthetic vertex at depth 0.5 in little-endian words.
pub fn vertex(x: f32, y: f32, colour: u32, u: f32, v: f32) -> String {
    hex(&[
        x.to_bits(),
        y.to_bits(),
        0.5f32.to_bits(),
        colour,
        u.to_bits(),
        v.to_bits(),
    ])
}
