// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 4: `vsh::run` against the Python oracle `tools/nv2a/interp.py` on 150 FABRICATED random programs
//! (`tests/data/vsh_oracle_cases.txt`, made by `python3 -m tools.t2024_vsh_diff --count 150 --seed 20241011`, no retail bytes).
//! Every output register, every component, bit for bit (NaN equals NaN). The big run is `examples/vsh_diff.rs`.
use tsfp_wgpu::vsh::{self, CONSTANT_COUNT, INPUT_COUNT, OUTPUT_COUNT, ProgramMemory};

fn word(text: &str, index: usize) -> u32 {
    u32::from_str_radix(&text[index * 8..index * 8 + 8], 16).unwrap()
}

#[test]
fn matches_python_oracle_bit_for_bit() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/vsh_oracle_cases.txt"
    ))
    .unwrap();
    let mut cases = 0;
    let mut failures = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let parts: Vec<&str> = line.split('|').collect();
        let mut memory = ProgramMemory::default();
        let bytes: Vec<u8> = (0..parts[0].len() / 2)
            .map(|i| u8::from_str_radix(&parts[0][i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        for (slot, chunk) in bytes.chunks(16).enumerate() {
            for i in 0..4 {
                memory.slots[slot][i] =
                    u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
        }
        let mut inputs = [[0.0f32; 4]; INPUT_COUNT];
        for (i, v) in inputs.iter_mut().flatten().enumerate() {
            *v = f32::from_bits(word(parts[1], i));
        }
        let mut constants = [[0.0f32; 4]; CONSTANT_COUNT];
        for entry in parts[2].split(';').filter(|e| !e.is_empty()) {
            let (slot, list) = entry.split_once('=').unwrap();
            for (c, v) in list.split(',').enumerate() {
                constants[slot.parse::<usize>().unwrap()][c] =
                    f32::from_bits(u32::from_str_radix(v, 16).unwrap());
            }
        }
        let out = vsh::run(&memory, 0, &inputs, &constants);
        for r in 0..OUTPUT_COUNT {
            for c in 0..4 {
                let expected = f32::from_bits(word(parts[3], r * 4 + c));
                let got = out.registers[r][c];
                let same =
                    got.to_bits() == expected.to_bits() || (got.is_nan() && expected.is_nan());
                if !same {
                    failures.push(format!(
                        "case {number} o{r}.{c}: rust {got:e} python {expected:e}"
                    ));
                }
            }
        }
        cases += 1;
    }
    assert_eq!(cases, 150);
    assert!(failures.is_empty(), "{failures:?}");
}
