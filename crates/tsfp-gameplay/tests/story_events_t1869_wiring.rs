//! T1869: `PortedResource` and `PortedShape` wired under the pool support (`svc_wiring::with_pool_layers`). A synthetic
//! P5CK image (FABRICATED, built here, no disc data) is mounted through the ported mount path `0x62040`; the model
//! loaders of `svc_pool` then run over the real resource layer (CRC32 key, pak lookup, `0x8E420`, `0x8DF60`).
//! The engine below (table relocators `0x8DD10`, allocator, formatter) stays recorded.
use std::cell::RefCell;

use tsfp_gameplay::story_events::svc_pool::{model_find, model_load_b};
use tsfp_gameplay::story_events::svc_resource::mount_memory;
use tsfp_gameplay::story_events::svc_wiring::{GuestRam, TraceEvent, with_pool_layers};

const IMAGE: u32 = 0x0200_0000;
const NAME: u32 = 0x0300_0000;

fn put_name(ram: &mut GuestRam, at: u32, name: &str) {
    for (k, byte) in name.bytes().enumerate() {
        ram.set_byte(at + k as u32, byte);
    }
    ram.set_byte(at + name.len() as u32, 0);
}

/// Model file: header (+0 = names offset, +4 = block offset, +0x28 = table offset), block table with the record
/// offset for mode 1 at `block + 0xB4`, one name record, one table row with a name list in its `+0xC` link.
fn model_file() -> Vec<u8> {
    let mut file = vec![0u8; 0x400];
    let word = |file: &mut Vec<u8>, at: usize, value: u32| {
        file[at..at + 4].copy_from_slice(&value.to_le_bytes());
    };
    word(&mut file, 0, 0x100);
    word(&mut file, 4, 0x40);
    word(&mut file, 0x28, 0x300);
    word(&mut file, 0x40 + 0xB4, 0x200);
    word(&mut file, 0x100 + 0x10, 0xFFFF_FFFF); // names: one record, then the terminator
    word(&mut file, 0x300, 0xFFFF_FFFF); // row 0: single indices -1 (skipped)
    word(&mut file, 0x304, 0xFFFF_FFFF);
    word(&mut file, 0x308, 0xFFFF_FFFF);
    word(&mut file, 0x30C, 0x380); // row 0 link +0xC: name list at 0x380
    word(&mut file, 0x310, 5);
    word(&mut file, 0x31C, 0x40); // row flag 0x40: the +0xC link is a list
    word(&mut file, 0x340, 0xFFFF_FFFF); // row 1 is the terminator row
    word(&mut file, 0x380, 1); // list: one entry, name index 0
    file
}

fn pak(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut image = vec![0u8; 16];
    image.extend_from_slice(payload);
    let offset = 16u32;
    let mut toc = Vec::new();
    toc.extend_from_slice(&tsfp_format_core::crc32(name.as_bytes()).to_le_bytes());
    toc.extend_from_slice(&offset.to_le_bytes());
    toc.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    toc.extend_from_slice(&0u32.to_le_bytes());
    image.extend_from_slice(&toc);
    image[0..4].copy_from_slice(b"P5CK");
    image[8..12].copy_from_slice(&(toc.len() as u32).to_le_bytes());
    image
}

fn mounted() -> GuestRam {
    let mut ram = GuestRam::default();
    let image = pak("hero.mdl", &model_file());
    for (k, byte) in image.iter().enumerate() {
        ram.set_byte(IMAGE + k as u32, *byte);
    }
    assert_eq!(mount_memory(&mut ram, IMAGE, image.len() as u32, 7), 0);
    // boot values of the search cells: excluded, forced and the two MRU slots all -1
    for cell in [0x4D_19EC, 0x4D_19F0, 0x4D_19F4, 0x4D_19F8] {
        ram.set_word(cell, 0xFFFF_FFFF);
    }
    put_name(&mut ram, NAME, "hero.mdl");
    ram
}

fn callees(trace: &RefCell<Vec<TraceEvent>>) -> Vec<u32> {
    trace
        .borrow()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::DestroyCallee { address, .. } => Some(*address),
            _ => None,
        })
        .collect()
}

#[test]
fn plain_loader_finds_the_mounted_pak_entry_in_place() {
    let mut ram = mounted();
    let trace = RefCell::new(Vec::new());
    let record = with_pool_layers(&trace, |pool| model_load_b(&mut ram, pool, NAME, 1));
    let file = IMAGE + 16;
    // the pak entry is used in place (resident, uncompressed, aligned); the record is file + 0x200
    assert_eq!(record, file + 0x200);
    assert_eq!(ram.word(record + 4), 1);
    // lookup, finish, bind, the table relocator and the stream classifier all ran ported: nothing reached the engine
    assert!(callees(&trace).is_empty(), "{:x?}", callees(&trace));
    // 0x8DD10 rebased the row link onto the file and flagged the referenced name record (byte +0xE |= 0x20)
    assert_eq!(ram.word(file + 0x30C), file + 0x380);
    assert_eq!(ram.byte(file + 0x100 + 0xE) & 0x20, 0x20);
    assert_eq!(
        ram.word(record + 0x10),
        file + 0x100,
        "names pointer of the record"
    );
    assert_eq!(
        ram.word(record + 0x14),
        file + 0x300,
        "table pointer of the record"
    );
}

#[test]
fn mode_zero_and_unknown_names_stay_inert() {
    let mut ram = mounted();
    let trace = RefCell::new(Vec::new());
    assert_eq!(
        with_pool_layers(&trace, |pool| model_load_b(&mut ram, pool, NAME, 0)),
        0
    );
    assert!(callees(&trace).is_empty());
}

#[test]
fn model_finder_reuses_the_cache_free_path_for_a_plain_model() {
    // a plain (non archive) model id: 0x96410 goes through 0x62C30 and binds once
    let mut ram = mounted();
    let trace = RefCell::new(Vec::new());
    let entry = 0x5D00_0000u32;
    let block = with_pool_layers(&trace, |pool| model_find(&mut ram, pool, entry, NAME, 0));
    assert_eq!(block, IMAGE + 16 + 0x40);
    assert_ne!(ram.word(block + 0xC) & 0x80_0000, 0, "marked loaded");
    let reached = callees(&trace);
    assert!(!reached.contains(&0x6_2C30) && !reached.contains(&0x8_DF60));
}
