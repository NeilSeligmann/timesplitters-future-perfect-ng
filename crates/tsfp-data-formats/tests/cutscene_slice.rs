//! T1908 Tier 0 smoke checks with bounded byte-addressable guest memory.
use tsfp_data_formats::cutscenes::{CutsceneProp, CutsceneState, KeyNodeProvider};
use tsfp_engine_memory::{GuestMemory, MappedMemory};
use tsfp_types::GuestAddr;
fn put(m: &mut MappedMemory<'_>, a: u32, v: u32) {
    m.write_u32(GuestAddr(a), v).unwrap();
}
fn get(m: &mut MappedMemory<'_>, a: u32) -> u32 {
    m.read_u32(GuestAddr(a)).unwrap()
}
#[test]
fn prop_reset_preserves_neighbours_and_state_marks_owner() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    let p = 0x10000;
    put(&mut m, p, 0x20000);
    m.write(GuestAddr(p + 0x1f4), &[7; 12]).unwrap();
    let prop = CutsceneProp(GuestAddr(p));
    CutsceneState { memory: &mut m }.reset_prop(prop).unwrap();
    let mut b = [0; 12];
    m.read(GuestAddr(p + 0x1f4), &mut b).unwrap();
    assert_eq!(b, [7, 7, 0, 0, 255, 255, 0, 0, 0, 0, 7, 7]);
    put(&mut m, 0x20018, 0x20);
    CutsceneState { memory: &mut m }
        .set_prop_state(prop, 3, 999, 4, 5)
        .unwrap();
    m.read(GuestAddr(p + 0x1f8), &mut b[..3]).unwrap();
    assert_eq!(&b[..3], &[3, 4, 5]);
    assert_eq!(get(&mut m, 0x20018), 0x24);
    put(&mut m, p, 0x900000);
    assert!(
        CutsceneState { memory: &mut m }
            .set_prop_state(prop, 6, 0, 7, 8)
            .is_err()
    );
    m.read(GuestAddr(p + 0x1f8), &mut b[..3]).unwrap();
    assert_eq!(&b[..3], &[6, 7, 8]);
}
#[test]
fn signed_table_lookup_and_bounds() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    m.write(GuestAddr(0x4d1ac6 + 12 * 0x44), &(-7i16).to_le_bytes())
        .unwrap();
    m.write(GuestAddr(0x4d1acc + 12 * 0x44), &65534u16.to_le_bytes())
        .unwrap();
    m.write(GuestAddr(0x4d1ace + 12 * 0x44), &123u16.to_le_bytes())
        .unwrap();
    let mut s = CutsceneState { memory: &mut m };
    assert_eq!(s.entry_index(-7).unwrap(), 12);
    assert_eq!(s.entry_index(65529).unwrap(), -1);
    assert_eq!(s.entry_pointer(-7).unwrap().0, 0x4d1ac0 + 12 * 0x44);
    assert_eq!(s.entry_word(0, -7).unwrap(), 65534);
    assert_eq!(s.entry_word(99, -7).unwrap(), 123);
    assert_eq!(s.entry_word(0, 99).unwrap(), 0);
    assert_eq!(s.slot_word(-1).unwrap(), u32::MAX);
    assert_eq!(s.slot_word(20).unwrap(), u32::MAX);
    put(&mut m, 0x51fc48 + 19 * 36, 456);
    assert_eq!(CutsceneState { memory: &mut m }.slot_word(19).unwrap(), 456);
}
#[test]
fn all_entry_rows_round_trip_signed_keys_and_both_payloads() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    for row in 0..13u32 {
        let base = 0x4d1ac0 + row * 0x44;
        let key = (row as i16) - 6;
        m.write(GuestAddr(base + 6), &key.to_le_bytes()).unwrap();
        m.write(GuestAddr(base + 12), &(1000 + row as u16).to_le_bytes())
            .unwrap();
        m.write(GuestAddr(base + 14), &(2000 + row as u16).to_le_bytes())
            .unwrap();
    }
    let mut s = CutsceneState { memory: &mut m };
    for row in 0..13u32 {
        let key = row as i32 - 6;
        assert_eq!(s.entry_index(key).unwrap(), row as i32);
        assert_eq!(s.entry_pointer(key).unwrap().0, 0x4d1ac0 + row * 0x44);
        assert_eq!(s.entry_word(0, key).unwrap(), 1000 + row);
        assert_eq!(s.entry_word(1, key).unwrap(), 2000 + row);
    }
    assert_eq!(
        s.story_campaign_level_ids().unwrap(),
        core::array::from_fn(|row| row as i16 - 6)
    );
    for missing in [-32769, -7, 7, 32768] {
        assert_eq!(s.entry_index(missing).unwrap(), -1);
        assert_eq!(s.entry_pointer(missing).unwrap().0, 0);
        assert_eq!(s.entry_word(0, missing).unwrap(), 0);
    }
}

#[test]
fn sequence_selection_retains_immediate_predecessor() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    put(&mut m, 0x7de314, 0xdead_beef);
    for next in [0, 1, u32::MAX, 0x8000_0000, 17] {
        let previous = get(&mut m, 0x7de314);
        CutsceneState { memory: &mut m }
            .select_sequence(next)
            .unwrap();
        assert_eq!(get(&mut m, 0x7de310), previous);
        assert_eq!(get(&mut m, 0x7de314), next);
    }
}
#[test]
fn stream_cache_resolution_hits_clamp_and_window_fallback() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    for (a, v) in [
        (0x7de530, 0x10000),
        (0x7bb260, 4),
        (0x7bb284, 2),
        (0x6d783c, 0x20000),
        (0x6d7818, 0x30000),
        (0x30004, 0x100),
        (0x4d19cc, 999),
        (0x6d7834, 0x40000),
        (0x40000, 1),
        (0x40004, 3),
        (0x6d7838, 0x50000),
        (0x30014, 0x80),
        (0x5000c, 0x30),
        (0x2001c, 0x20),
    ] {
        put(&mut m, a, v);
    }
    let mut s = CutsceneState { memory: &mut m };
    assert_eq!(s.key_node(999, 1).unwrap().0, 0x10050); // clamps to frame 3
    // Cached hit must avoid the now invalid offset table.
    put(&mut m, 0x6d783c, 0x900000);
    assert_eq!(
        CutsceneState { memory: &mut m }.key_node(3, 1).unwrap().0,
        0x10050
    );
    put(&mut m, 0x7de520, 1); // window not resident -> original header fallback
    assert_eq!(
        CutsceneState { memory: &mut m }.frame_pointer(3).unwrap(),
        0x30100
    );
    assert_eq!(
        CutsceneState { memory: &mut m }.frame_pointer(0).unwrap(),
        0x30100
    );
    put(&mut m, 0x6d783c, 0x20000);
    put(&mut m, 0x20004, 0x10);
    assert_eq!(
        CutsceneState { memory: &mut m }.key_node(-2, 1).unwrap().0,
        0x30110
    );
    CutsceneState { memory: &mut m }
        .reset_stream_state()
        .unwrap();
    assert_eq!(get(&mut m, 0x6d7840), 1);
    assert_eq!(get(&mut m, 0x6d786c), u32::MAX);
    assert_eq!(
        CutsceneState { memory: &mut m }
            .stream_state_mask()
            .unwrap(),
        0
    );
    put(&mut m, 0x6d7840, 0x11);
    assert_eq!(
        CutsceneState { memory: &mut m }
            .stream_state_mask()
            .unwrap(),
        16
    );
}
#[test]
fn selection_track_and_active_record_operations() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    m.write(GuestAddr(0x7e03c0), &(-1i16).to_le_bytes())
        .unwrap();
    let mut s = CutsceneState { memory: &mut m };
    s.set_track_pair(0, 11, 12).unwrap();
    s.clear_stream_pair().unwrap();
    s.select_sequence(15).unwrap();
    s.select_sequence(16).unwrap();
    s.clear_sequence_gate().unwrap();
    assert!(s.sequence_gate_clear().unwrap());
    s.invalidate_subtitle().unwrap();
    s.set_slot_pair(5, 6).unwrap();
    s.invalidate_sequence_index().unwrap();
    s.reset_sequence_selection().unwrap();
    assert_eq!(s.current_sequence().unwrap(), u32::MAX);
    assert_eq!(get(&mut m, 0x7de658), 11);
    assert_eq!(get(&mut m, 0x7de65c), 12);
    assert_eq!(get(&mut m, 0x7de310), 15);
    assert_eq!(get(&mut m, 0x7de314), 16);
    assert_eq!(get(&mut m, 0x7bb248), u32::MAX);
    assert_eq!(get(&mut m, 0x7490f0), u32::MAX);
    assert_eq!(get(&mut m, 0x6f73d4), 6);
    assert_eq!(get(&mut m, 0x6b83e4), 0);
    assert_eq!(get(&mut m, 0x75ce58), u32::MAX);
    assert_eq!(get(&mut m, 0x75ce5c), 0);
    CutsceneState { memory: &mut m }.request_stop().unwrap();
    put(&mut m, 0x70e164, 0x10000);
    put(&mut m, 0x7497a8, 999);
    assert_eq!(
        CutsceneState { memory: &mut m }.active_record().unwrap().0,
        0x10000
    );
    CutsceneState { memory: &mut m }.request_stop().unwrap();
    CutsceneState { memory: &mut m }.restart_active().unwrap();
    assert_eq!(get(&mut m, 0x1001c), 1);
    assert_eq!(get(&mut m, 0x10004), 999);
    put(&mut m, 0x52ec8c + 5 * 24, 456);
    assert_eq!(
        CutsceneState { memory: &mut m }
            .file_entry_field(5)
            .unwrap(),
        456
    );
}
