//! T1668 tier 1: `0x96A40` allocation plan and `0x965F0` arena size against the ORIGINAL (`tools/t1668_buffers_gold.py`).
use tsfp_engine_anim::buffers::{instance_arena_size, slot_index, track_buffer_plan};
use tsfp_engine_anim::keyrec::records_byte_size;

mod support;
use support::Rng;

#[test]
fn buffer_plan_and_arena_size_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/buffers.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut plans, mut sizes) = (0, 0);
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let mut r = Rng(f[1].parse().unwrap());
        let parts = r.below(5) as u32 + 1;
        let nodes = r.below(6) as u32 + 1;
        let part = r.below(u64::from(parts)) as u32;
        let lod = r.below(3) as u32;
        let mask = r.below(64) as u32;
        let k = r.below(4) as usize + 1;
        let mut groups = Vec::new();
        for _ in 0..k {
            let mut rec: Vec<u8> = (0..10).map(|_| r.below(256) as u8).collect();
            rec[3] = r.below(128) as u8;
            groups.extend(rec);
        }
        let mut term: Vec<u8> = (0..10).map(|_| r.below(256) as u8).collect();
        term[3] = 0x80 + r.below(128) as u8;
        groups.extend(term);
        if f[0] == "B" {
            assert_eq!(instance_arena_size(nodes, parts).to_string(), f[2], "{line}");
            sizes += 1;
            continue;
        }
        let vertices = records_byte_size(&groups);
        let plan = track_buffer_plan(vertices, k as u32, mask);
        let (sizes_text, entry_text) = line[3 + f[1].len()..].split_once(" | ").unwrap();
        let got: Vec<String> = plan.iter().map(|a| a.size.to_string()).collect();
        let want: Vec<&str> = if sizes_text.is_empty() { vec![] } else { sizes_text.split(',').collect() };
        assert_eq!(got, want, "{line}");
        // each allocation returns 0x10000 * call number; field 8 gets +2 when the last plan step (bit 5) wrote it
        let mut entry = [0u32; 6];
        for (i, a) in plan.iter().enumerate() {
            let address = 0x10000 * (i as u32 + 1);
            entry[a.field as usize / 4] = if a.field == 8 && mask & 0x20 != 0 && i == plan.len() - 1 { address + 2 } else { address };
        }
        let want_entry: Vec<u32> = entry_text.split(' ').map(|w| u32::from_str_radix(w, 16).unwrap()).collect();
        assert_eq!(entry.to_vec(), want_entry, "{line}");
        assert_eq!(slot_index(part, lod), lod + 3 * part);
        plans += 1;
    }
    assert!(plans > 300 && sizes > 300, "{plans} {sizes}");
}
