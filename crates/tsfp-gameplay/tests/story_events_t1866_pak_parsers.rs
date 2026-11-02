//! T1866: the original's resident pak lookup (`svc_resource`) against the Rust PAK parser (`tsfp-format-pak`):
//! a synthetic P5CK image (no disc data) is parsed by `Pak::parse` and mounted into guest memory the way the
//! original mounts a memory resident pak (`0x62040`); `pak_find` / `find_file` / `file_size` then agree with the
//! parser on keys, first-match order for duplicate keys, offsets, sizes and payload bytes.
//! Confidence: MEASURED for the lookup (goldens in `story_events_t1866_resource.rs`), the parser side is the
//! T1761 contract.
use std::collections::BTreeMap;
use tsfp_format_pak::Pak;
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_resource::*;

const IMAGE: u32 = 0x0200_0000;
const NAME: u32 = 0x0300_0000;

#[derive(Default)]
struct Mem {
    bytes: BTreeMap<u32, u8>,
}

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        self.bytes.get(&address).copied().unwrap_or(0)
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
    }
    fn read_u32(&self, address: u32) -> u32 {
        (0..4).fold(0, |acc, k| {
            acc | u32::from(self.read_u8(address + k)) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        for k in 0..4 {
            self.write_u8(address + k, (value >> (8 * k)) as u8);
        }
    }
}

struct Nothing;

impl DestroyEffects for Nothing {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, _: &[u32]) -> u32 {
        panic!("unexpected engine call {address:#x}");
    }
}

/// A P5CK image: header, 16 byte aligned payloads, TOC at the end. `rows` = (name, payload, stored size).
fn build(rows: &[(&str, Vec<u8>, u32)]) -> Vec<u8> {
    let mut image = vec![0u8; 12];
    let mut toc = Vec::new();
    for (name, payload, stored) in rows {
        while image.len() % 16 != 0 {
            image.push(0);
        }
        toc.extend_from_slice(&tsfp_format_core::crc32(name.as_bytes()).to_le_bytes());
        toc.extend_from_slice(&(image.len() as u32).to_le_bytes());
        toc.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        toc.extend_from_slice(&stored.to_le_bytes());
        image.extend_from_slice(payload);
    }
    let toc_at = image.len() as u32;
    image[..4].copy_from_slice(b"P5CK");
    image[4..8].copy_from_slice(&toc_at.to_le_bytes());
    image[8..12].copy_from_slice(&(toc.len() as u32).to_le_bytes());
    image.extend_from_slice(&toc);
    image
}

fn put_name(memory: &mut Mem, text: &str) {
    for (k, byte) in text.bytes().enumerate() {
        memory.write_u8(NAME + k as u32, byte);
    }
    memory.write_u8(NAME + text.len() as u32, 0);
}

#[test]
fn crc32_agrees_with_the_format_core() {
    let mut memory = Mem::default();
    for text in [
        "",
        "a",
        "assets/x/y.bin",
        "textures/cc95c8.xbt",
        "host0:data/a",
    ] {
        put_name(&mut memory, text);
        assert_eq!(
            crc32_string(&memory, NAME),
            if text.is_empty() {
                0
            } else {
                tsfp_format_core::crc32(text.as_bytes())
            }
        );
        assert_eq!(
            crc32_bytes(text.as_bytes()),
            if text.is_empty() {
                0
            } else {
                tsfp_format_core::crc32(text.as_bytes())
            }
        );
    }
}

#[test]
fn resident_pak_lookup_agrees_with_the_pak_parser() {
    let rows: Vec<(&str, Vec<u8>, u32)> = vec![
        ("data/one.bin", (0..40u8).collect(), 0),
        ("data/two.bin", vec![7; 17], 0),
        ("data/one.bin", vec![9; 5], 0), // duplicate key: the first row wins in both
        ("data/packed.bin", vec![1; 64], 12),
    ];
    let image = build(&rows);
    let pak = Pak::parse(&image).expect("parse");
    let mut memory = Mem::default();
    for (k, byte) in image.iter().enumerate() {
        memory.write_u8(IMAGE + k as u32, *byte);
    }
    memory.write_u32(PAK_COUNT, 0);
    for cell in [PAK_SKIP_SLOT, PAK_FORCED_SLOT, PAK_MRU_A, PAK_MRU_B] {
        memory.write_u32(cell, 0xFFFF_FFFF);
    }
    let slot = mount_memory(&mut memory, IMAGE, image.len() as u32, 7);
    assert_eq!(slot, 0);
    let toc = IMAGE + image.len() as u32 - 16 * pak.entries.len() as u32;
    let mut effects = Nothing;
    for (name, _, _) in &rows {
        put_name(&mut memory, name);
        let key = tsfp_format_core::crc32(name.as_bytes());
        let first = pak
            .entries
            .iter()
            .position(|entry| entry.key == key)
            .expect("row");
        let entry = pak_find(&mut memory, key);
        assert_eq!(entry, toc + 16 * first as u32, "{name}: first matching row");
        let parsed = &pak.entries[first];
        assert_eq!(memory.read_u32(entry), parsed.key);
        assert_eq!(memory.read_u32(entry + 4), parsed.offset);
        assert_eq!(memory.read_u32(entry + 8), parsed.decoded_size);
        assert_eq!(memory.read_u32(entry + 0xC), parsed.stored_size);
        let size_cell = NAME + 0x100;
        let direct = find_file(&mut memory, NAME, size_cell);
        if parsed.stored_size == 0 && parsed.offset % 16 == 0 {
            assert_eq!(direct, IMAGE + parsed.offset, "{name}: direct pointer");
            assert_eq!(memory.read_u32(size_cell), parsed.decoded_size);
            let payload = pak.payload(first).expect("payload");
            for (k, byte) in payload.iter().enumerate() {
                assert_eq!(memory.read_u8(direct + k as u32), *byte);
            }
        } else {
            assert_eq!(direct, 0, "{name}: compressed entries are never direct");
        }
        let expected = parsed.decoded_size + if parsed.stored_size != 0 { 0x100 } else { 0 };
        assert_eq!(
            file_size(&mut memory, &mut effects, NAME),
            expected,
            "{name}: size"
        );
    }
    // the device prefix is cut before hashing
    put_name(&mut memory, "host0:data/two.bin");
    assert_ne!(find_file(&mut memory, NAME, 0), 0);
    // an unknown name falls through to the file system (an engine call)
    put_name(&mut memory, "data/unknown.bin");
    assert_eq!(find_file(&mut memory, NAME, 0), 0);
    assert_eq!(pak_find(&mut memory, 0x1234_5678), 0);
}
