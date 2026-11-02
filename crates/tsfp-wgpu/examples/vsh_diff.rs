// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 4: differential of `vsh::run` against the Python oracle `tools/nv2a/interp.py`.
//! Reads the cases `python3 -m tools.t2024_vsh_diff` writes and compares every output (bit for bit, or within N ulps, NaN
//! equals NaN).
//! `cargo run --release --manifest-path crates/tsfp-wgpu/Cargo.toml --example vsh_diff -- cases.txt [--tolerance-ulps N]`
use std::collections::BTreeMap;
use tsfp_wgpu::vsh::{self, CONSTANT_COUNT, INPUT_COUNT, OUTPUT_COUNT, ProgramMemory};

fn word(text: &str, index: usize) -> u32 {
    u32::from_str_radix(&text[index * 8..index * 8 + 8], 16).unwrap()
}

fn close(left: f32, right: f32, ulps: u32) -> bool {
    if left.is_nan() || right.is_nan() {
        return left.is_nan() && right.is_nan();
    }
    if left.to_bits() == right.to_bits() {
        return true;
    }
    let order = |v: f32| {
        let b = i64::from(v.to_bits());
        if b & 0x8000_0000 != 0 {
            -(b & 0x7FFF_FFFF)
        } else {
            b
        }
    };
    (order(left) - order(right)).unsigned_abs() <= u64::from(ulps)
}

fn main() {
    let mut path = None;
    let mut ulps = 0u32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--tolerance-ulps" {
            ulps = args.next().unwrap().parse().unwrap();
        } else {
            path = Some(arg);
        }
    }
    let text = std::fs::read_to_string(path.expect("cases file")).unwrap();
    let (mut total, mut bad_cases, mut bad_values, mut values) = (0u32, 0u32, 0u64, 0u64);
    let mut by_op: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for (line_number, line) in text.lines().enumerate() {
        let parts: Vec<&str> = line.split('|').collect();
        let program: Vec<u8> = (0..parts[0].len() / 2)
            .map(|i| u8::from_str_radix(&parts[0][i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        let mut memory = ProgramMemory::default();
        let mut ops = Vec::new();
        for (slot, chunk) in program.chunks(16).enumerate() {
            let mut words = [0u32; 4];
            for (i, w) in words.iter_mut().enumerate() {
                *w = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
            memory.slots[slot] = words;
            if let Some(inst) = vsh::decode(words) {
                ops.push(format!("mac{:02}", inst.mac));
                ops.push(format!("ilu{}", inst.ilu));
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
        let mut case_bad = false;
        for r in 0..OUTPUT_COUNT {
            for c in 0..4 {
                let expected = f32::from_bits(word(parts[3], r * 4 + c));
                values += 1;
                if !close(out.registers[r][c], expected, ulps) {
                    bad_values += 1;
                    if !case_bad && bad_cases < 8 {
                        println!(
                            "case {line_number} o{r}.{c}: rust {:e} ({:08x}) python {:e} ({:08x}) ops {ops:?}",
                            out.registers[r][c],
                            out.registers[r][c].to_bits(),
                            expected,
                            expected.to_bits()
                        );
                    }
                    case_bad = true;
                }
            }
        }
        total += 1;
        ops.sort();
        ops.dedup();
        for op in ops {
            let entry = by_op.entry(op).or_default();
            entry.0 += 1;
            entry.1 += u32::from(case_bad);
        }
        bad_cases += u32::from(case_bad);
    }
    println!(
        "cases {total} mismatching {bad_cases} values {values} mismatching {bad_values} (tolerance {ulps} ulp)"
    );
    for (op, (n, bad)) in by_op {
        println!("  {op}: cases containing it {n}, mismatching {bad}");
    }
}
