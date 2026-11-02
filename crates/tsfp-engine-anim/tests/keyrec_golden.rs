//! T1668 tier 1: `0x606E0 0x60740 0x949C0 0x8EA20` against the ORIGINAL (`tools/t1668_keyrec_gold.py`, worlds regenerated from the seed).
use tsfp_engine_anim::keyrec::*;
use tsfp_engine_anim::start::Layer;

mod support;
use support::Rng;

fn store_world(r: &mut Rng, allow_stream: bool) -> (Vec<u32>, i32, bool) {
    let mut words: Vec<u32> = (0..0x200).map(|_| r.below(1 << 32) as u32).collect();
    let q = 4 + r.below(8) as usize;
    words[2] = (4 * q) as u32 + r.below(4) as u32;
    words[q] = r.below(3) as u32;
    for i in 1..1 + 3 * 16 + 20 {
        words[q + i] = r.below(3) as u32;
    }
    let count = r.below(20) as i32 + 1;
    let streamed = r.below(3) == 0 && allow_stream;
    (words, count, streamed)
}

#[test]
fn key_record_helpers_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/keyrec.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for line in text.lines() {
        let (head, result) = line.split_once(' ').and_then(|(f, rest)| rest.split_once(' ').map(|(seed, res)| ((f, seed), res))).unwrap();
        let function = u32::from_str_radix(head.0, 16).unwrap();
        let mut r = Rng(head.1.parse().unwrap());
        match function {
            0x606E0 => {
                let (store, count, streamed) = store_world(&mut r, true);
                let frame = r.below(40) as i32 - 5;
                let stream = |f: i32| (f as u32).wrapping_mul(7).wrapping_add(0x1000);
                let got = match key_record(&store, count, frame, streamed.then_some(&stream as &dyn Fn(i32) -> u32)) {
                    KeyRecord::Store(offset) => {
                        // the generator reports absolute pointers outside the 8 KB store as `s<pointer>`
                        let absolute = offset.wrapping_add(0x02D0_0000);
                        if offset < 0x2000 { format!("{offset:x}") } else { format!("s{absolute:x}") }
                    }
                    KeyRecord::Streamed(value) => format!("s{value:x}"),
                };
                assert_eq!(got, result, "{head:?}");
            }
            0x60740 => {
                let (store, count, _) = store_world(&mut r, false);
                let frame = r.below(40) as i32 - 5;
                let outs: Vec<bool> = (0..3).map(|_| r.below(2) != 0).collect();
                let KeyRecord::Store(offset) = key_record(&store, count, frame, None) else { unreachable!() };
                let base = offset as usize / 4;
                let (a, b, c) = key_fields(&store[base..base + 8]);
                let mut out = [0x7777u32; 16];
                if outs[0] {
                    out[0..4].copy_from_slice(&a);
                }
                if outs[1] {
                    out[8..11].copy_from_slice(&b);
                }
                if outs[2] {
                    out[12] = c;
                }
                let want: Vec<u32> = result.split(' ').map(|w| u32::from_str_radix(w, 16).unwrap()).collect();
                assert_eq!(out.to_vec(), want, "{head:?}");
            }
            0x949C0 => {
                let k = r.below(5) as usize;
                let mut data = Vec::new();
                for _ in 0..k {
                    let mut rec: Vec<u8> = (0..10).map(|_| r.below(256) as u8).collect();
                    rec[3] = r.below(128) as u8;
                    data.extend(rec);
                }
                let mut term: Vec<u8> = (0..10).map(|_| r.below(256) as u8).collect();
                term[3] = 0x80 + r.below(128) as u8;
                data.extend(term);
                let null = r.below(6) == 0;
                let got = if null { 0 } else { records_byte_size(&data) };
                assert_eq!(format!("{got:x}"), result, "{head:?}");
            }
            0x8EA20 => {
                let ident = r.below(30) as u32;
                let slot = r.below(1 << 15) as i64 - (1 << 14);
                let speed = [0.0f32, 1.0, -2.5, 0.125][r.below(4) as usize];
                let mut a = Layer(std::array::from_fn(|_| r.below(1 << 32) as u32));
                let mut b = Layer(std::array::from_fn(|_| r.below(1 << 32) as u32));
                let (a0, b0) = (a.clone(), b.clone());
                init_layers(&mut a, &mut b, ident, slot as i32 as u32, speed);
                let diff = |new: &Layer, old: &Layer| {
                    let parts: Vec<String> = (0..22).filter(|&i| new.0[i] != old.0[i]).map(|i| format!("{i}:{:x}", new.0[i])).collect();
                    if parts.is_empty() { "-".to_string() } else { parts.join(",") }
                };
                assert_eq!(format!("{} {}", diff(&a, &a0), diff(&b, &b0)), result, "{head:?}");
            }
            other => panic!("{other:x}"),
        }
        *counts.entry(function).or_insert(0usize) += 1;
    }
    assert_eq!(counts.len(), 4);
}
