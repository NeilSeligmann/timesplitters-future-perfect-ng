// SPDX-License-Identifier: GPL-3.0-or-later

pub fn hex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

pub fn word(text: &str) -> u32 {
    u32::from_str_radix(text, 16).unwrap()
}
