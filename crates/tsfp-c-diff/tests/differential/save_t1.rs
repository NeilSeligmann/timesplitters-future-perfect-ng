// SPDX-License-Identifier: GPL-3.0-or-later
//! T1899 Tier 1: data/save and mapmaker storage/content ports against the PRIVATE LIFTED ORIGINAL
//! (extracted into OUT_DIR at build time by `sv_lifted.rs`, nothing committed). Same method as
//! `modes_t1.rs`: identical random guest memory in a C arena and a Rust arena, EAX where defined,
//! the nested-call trace (callee and stack arguments) and the watched memory compared per seed.
//! Without the lifted tree the file compiles to nothing.
#![cfg(sv_lifted)]
use super::*;

use tsfp_gameplay::{
    save_device::{self, SaveDeviceHost, FormatArg},
    save_files::{self, FreeSpaceDialogHost, ContentFiles, SaveFileSystem, SaveReadFiles, SaveWriteFiles, SignatureHost, SlotEnumerationHost},
    save_profile::{self, ProfileHost, StatsHost},
    save_unlock::{self, UnlockHost},
};
use tsfp_gameplay::cutscene_sequence::{
    CutsceneDefinition, CutsceneSequence, KeyFrameRange, PlaybackTrack,
};
use tsfp_mapmaker::{chunks::{self, DeserializeHost}, content::{self, ContentHost, TilesetHost}, storage::{self, MapmakerSaveHost, StorageHost}};
use tsfp_format_profile::{settings::Settings, PlayerRecord};

unsafe extern "C" {
    fn sv_run(va: u32, regs: *mut u32) -> i32;
    fn sv_script(words: *const u32, entries: u32);
    fn sv_argc(va: u32) -> i32;
    fn sv_trace_len() -> u32;
    fn sv_trace_entry(index: u32) -> *const u32;
    fn sv_set_force_begin(on: i32);
    fn sv_set_slot(on: i32, value: u32);
}

const SEEDS: usize = 3000;
/// The upper 16 bits of the second packed-cell dword passed to 0x334b20 are uninitialised stack
/// padding in the original; `drive` masks them unless a test turns this off to prove the point.
static MASK_PADDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// Makes the signature begin stub (and the Rust host) always succeed, see `content_write` test.
static FORCE_BEGIN_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const POOL: u32 = 0x0090_0000;
const POOL_LEN: u32 = 0x4000;
const ARG_FRAME: u32 = 0x40;
const SLOT_TABLE: u32 = 0x007B_EDC0;
const SLOT_STRIDE: u32 = 0x1F54;
const DEVICES: u32 = 0x0056_55B0;

type Row = [u32; 4];

struct Host {
    view: View,
    script: Vec<[u32; 3]>,
    trace: Vec<Row>,
    /// T2130: fixed answer of the 0x356870 local-slot callee (the C stub does the same).
    slot: Option<u32>,
}
impl Host {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        let argc = unsafe { sv_argc(target) };
        assert!(argc >= 0, "unknown callee {target:x}");
        let mut row = [target, 0, 0, 0];
        let recorded = if matches!(target, 0x24F40 | 0x250C0 | 0x25180 | 0x25070 | 0x25020 | 0x251D0) { 3 } else { (argc as usize).min(3) };
        for (i, a) in args.iter().take(recorded).enumerate() {
            row[1 + i] = *a;
        }
        let index = self.trace.len();
        self.trace.push(row);
        if let (0x356870, Some(slot)) = (target, self.slot) {
            return slot;
        }
        self.script.get(index).map_or(0, |r| r[0])
    }
}
impl ProfileHost for Host {
    fn net_is_session_active(&mut self) -> bool {
        self.call(0x3533F0, &[]) != 0
    }
    fn local_slot_index(&mut self, player: u32) -> u32 {
        self.call(0x356800, &[player])
    }
    fn local_time_encode(&mut self) -> u32 {
        self.call(0x150230, &[])
    }
    fn string_copy(&mut self, _m: &mut dyn GameplayMemory, _d: u32, _s: u32, _l: u32) {
        unimplemented!()
    }
}
impl SaveDeviceHost for Host {
    fn signature_size_lookup(&mut self, class: u32) -> u32 {
        self.call(0x37E89E, &[class])
    }
    fn unmount_memory_unit(&mut self, port: u32, slot: u32) -> u32 {
        self.call(0x46D8F6, &[port, slot])
    }
    fn signature_open(&mut self, _c: u32) -> u32 { unimplemented!() }
    fn last_error(&mut self) -> u32 { unimplemented!() }
    fn arena_alloc(&mut self, _s: u32) -> u32 { unimplemented!() }
    fn text_sprintf(&mut self, _m: &mut dyn GameplayMemory, _d: u32, _i: u32, _a: &[FormatArg]) { unimplemented!() }
    fn text_copy(&mut self, _m: &mut dyn GameplayMemory, _d: u32, _i: u32) { unimplemented!() }
    fn text_len(&mut self, _i: u32) -> u32 { unimplemented!() }
    fn mount_memory_unit(&mut self, _p: u32, _s: u32) -> (u32, u8) { unimplemented!() }
    fn volume_label(&mut self, _l: u8) -> Option<Vec<u16>> { unimplemented!() }
    fn volume_serial(&mut self, _m: &mut dyn GameplayMemory, _l: u8, _d: u32) { unimplemented!() }
    fn delete_save_game(&mut self, _l: u8, _n: &[u16]) -> u32 { unimplemented!() }
    fn memory_unit_mask(&mut self) -> u32 { unimplemented!() }
    fn device_changes(&mut self) -> Option<(u32, u32)> { unimplemented!() }
    fn disk_space(&mut self, _l: u8) -> Option<(u64, u64)> { unimplemented!() }
    fn net_enter_live_mode(&mut self) { unimplemented!() }
}
impl StorageHost for Host {
    fn signature_size_lookup(&mut self, class: u32) -> u32 {
        self.call(0x37E89E, &[class])
    }
}
impl SaveFileSystem for Host {
    fn sprintf(&mut self, _m: &mut dyn GameplayMemory, dst: u32, format: u32, args: &[u32]) {
        self.call(0x3C85D4, &[dst, format, args.first().copied().unwrap_or(0)]);
    }
    fn find_first_save(&mut self, root: u32, find_data: u32) -> u32 {
        self.call(0x37E44D, &[root, find_data])
    }
    fn find_first_content(&mut self, root: u32, find_data: u32) -> u32 {
        self.call(0x37F512, &[root, 0, find_data])
    }
    fn find_next_save(&mut self, handle: u32, find_data: u32) -> u32 {
        self.call(0x37E557, &[handle, find_data])
    }
    fn find_next_content(&mut self, handle: u32, find_data: u32) -> u32 {
        self.call(0x37F662, &[handle, find_data])
    }
    fn find_close(&mut self, handle: u32) {
        self.call(0x37E59E, &[handle]);
    }
    fn open_for_read(&mut self, path: u32) -> u32 {
        self.call(0x37D231, &[path, 0x8000_0000, 1, 0, 3, 0, 0])
    }
    fn file_size(&mut self, handle: u32) -> u32 {
        self.call(0x37D1F6, &[handle, 0])
    }
    fn close_handle(&mut self, handle: u32) {
        self.call(0x37CBC4, &[handle]);
    }
    fn signature_size(&mut self) -> u32 {
        self.call(0x37E89E, &[0])
    }
    fn file_time_to_local(&mut self, source: u32, destination: u32) {
        self.call(0x37F0D6, &[source, destination]);
    }
    fn file_time_to_system(&mut self, source: u32, destination: u32) {
        self.call(0x37EB84, &[source, destination]);
    }
    fn pack_timestamp(&mut self, year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u32 {
        let _ = (hour, minute, second);
        self.call(0x150350, &[year, month, day])
    }
    fn entry_callback(&mut self, _callback: u32, context: u32, entry: u32) {
        self.call(0xC0DE0001, &[context, entry]);
    }
}
impl SlotEnumerationHost for Host {
    fn enumerate_slots(&mut self, _m: &mut dyn GameplayMemory, start: u32, callback: u32, context: u32) {
        self.call(0x24EC0, &[start, callback, context]);
    }
    fn bounded_copy(&mut self, _m: &mut dyn GameplayMemory, destination: u32, source: u32, limit: u32) {
        self.call(0x3C8960, &[destination, source, limit]);
    }
}
impl FreeSpaceDialogHost for Host {
    fn text_by_id(&mut self, id: u32) -> u32 {
        self.call(0x3D7B0, &[id])
    }
    fn selected_slot(&mut self) -> u32 {
        // 0x827f0 is a direct root of the lifted extract: no call row.
        save_device::memory_unit_slot_get(&self.view)
    }
    fn slot_global(&mut self) -> u32 {
        self.call(0x827C0, &[])
    }
    fn selected_slot_name(&mut self) -> u32 {
        self.call(0x82820, &[])
    }
    fn error_message(&mut self, _m: &mut dyn GameplayMemory, device: u32, code: u32, buffer: u32, kind: u32, subject: u32, text: u32) {
        let _ = (kind, subject, text);
        self.call(0x24950, &[device, code, buffer]);
    }
    fn sprintf_template(&mut self, _m: &mut dyn GameplayMemory, buffer: u32, template: u32, arg_a: u32, arg_b: u32) {
        let _ = arg_b;
        self.call(0x3D7F0, &[buffer, template, arg_a]);
    }
    fn free_space_query(&mut self, _m: &mut dyn GameplayMemory, device: u32, out_free: u32, out_total: u32) {
        self.call(0x25C30, &[device, out_free, out_total]);
    }
    fn blocks_for_size(&mut self, size: u32) -> u32 {
        self.call(0x25BE0, &[size])
    }
    fn show_message(&mut self, context: u32, text: u32, handler: u32) {
        self.call(0x82560, &[context, 0xFFFF_FFFF, text]);
        let _ = handler;
    }
    fn show_confirm(&mut self, context: u32, text: u32, yes_handler: u32, no_handler: u32) {
        self.call(0x825C0, &[context, 0xFFFF_FFFF, text]);
        let _ = (yes_handler, no_handler);
    }
}
impl SignatureHost for Host {
    fn signature_size_lookup(&mut self, class: u32) -> u32 {
        self.call(0x37E89E, &[class])
    }
    fn signature_begin(&mut self, m: &mut dyn GameplayMemory, state: u32, flags: u32) -> u32 {
        let status = self.call(0x24F40, &[state, 0, flags]);
        // The lifted stub fills the state like a successful begin: handle 7, size 4, class 1, buffer.
        m.write32(state, 7);
        m.write32(state + 4, 4);
        m.write32(state + 8, 1);
        m.write32(state + 0xC, 0x0090_0A00);
        if FORCE_BEGIN_OK.load(std::sync::atomic::Ordering::Relaxed) { 1 } else { status }
    }
    fn signature_finalize(&mut self, handle: u32, buffer: u32) {
        self.call(0x37E962, &[handle, buffer]);
    }
    fn signature_update(&mut self, handle: u32, data: u32, length: u32) {
        self.call(0x37E914, &[handle, data, length]);
    }
    fn signature_compare(&mut self, _m: &mut dyn GameplayMemory, computed: u32, stored: u32) -> u32 {
        self.call(0x250C0, &[computed, stored, 0])
    }
    fn signature_buffer_load(&mut self, _m: &mut dyn GameplayMemory, state: u32) {
        self.call(0x25180, &[0, state, 0]);
    }
    fn release_block(&mut self, block: u32) {
        self.call(0x3E1E0, &[block]);
    }
}
impl SaveWriteFiles for Host {
    fn free_space_query(&mut self, _m: &mut dyn GameplayMemory, device: u32, out_total: u32, out_free: u32) -> u32 {
        self.call(0x25C30, &[device, out_total, out_free])
    }
    fn last_error(&mut self) -> u32 {
        self.call(0x37E9A7, &[])
    }
    fn blocks_for_size(&mut self, size: u32) -> u32 {
        self.call(0x25BE0, &[size])
    }
    fn create_directory(&mut self, root: u32, wide_name: u32, _out: u32) -> u32 {
        self.call(0x37E11A, &[root, wide_name, 4])
    }
    fn create_file_for_write(&mut self, path: u32) -> u32 {
        self.call(0x37D231, &[path, 0x4000_0000, 0])
    }
    fn write_file(&mut self, file: u32, data: u32, length: u32, _written: u32) -> u32 {
        self.call(0x37CCF5, &[file, data, length])
    }
    fn append_signature(&mut self, _m: &mut dyn GameplayMemory, state: u32, file: u32) -> u32 {
        self.call(0x25070, &[state, file, 0])
    }
    fn flush_file(&mut self, file: u32) {
        self.call(0x37CDCB, &[file]);
    }
    fn delete_path(&mut self, root: u32, wide_name: u32) {
        self.call(0x37E37D, &[root, wide_name]);
    }
}
impl SaveReadFiles for Host {
    fn read_file(&mut self, file: u32, buffer: u32, length: u32, _read_out: u32) -> u32 {
        self.call(0x37CC08, &[file, buffer, length])
    }
    fn seek(&mut self, file: u32, offset: u32) {
        self.call(0x37CE4F, &[file, offset, 0]);
    }
    fn allocate_block(&mut self, size: u32) -> u32 {
        self.call(0x3DFA0, &[size, 0]);
        0x0094_0000
    }
    fn read_signature(&mut self, _m: &mut dyn GameplayMemory, state: u32, file: u32) -> u32 {
        self.call(0x25020, &[state, file, 0])
    }
}
impl ContentFiles for Host {
    fn create_content(&mut self, content_id: u32, title_id: u32, _out: u32) -> u32 {
        self.call(0x37F99F, &[0, content_id, title_id])
    }
    fn normalize_path(&mut self, path: u32) {
        self.call(0x29650, &[path]);
    }
    fn create_content_directory(&mut self, content_id: u32, title_id: u32, _wide: u32, _out: u32) -> u32 {
        self.call(0x37F9F1, &[0, content_id, title_id])
    }
    fn release_signature(&mut self, state: u32) {
        self.call(0x251D0, &[state, 0, 0]);
    }
    fn delete_content(&mut self, out: u32) {
        self.call(0x37FBA9, &[out]);
    }
}
impl TilesetHost for Host {
    fn fn_00331a20_tileset_group(&mut self, id: u32, c: u32) -> u32 {
        self.call(0x331A20, &[id, c]) & 1
    }
    fn fn_00331920_group_flag(&mut self, id: u32, c: u32) -> u32 {
        self.call(0x331920, &[id, c]) & 1
    }
    fn fn_00332490_object_base(&mut self, tag: u32, context: u32) -> u32 {
        0x3E9 + (self.call(0x332490, &[tag, context]) & 3)
    }
    fn fn_000630c0_resource_entry(&mut self, table: u32) -> u32 {
        self.call(0x630C0, &[table]);
        0x0090_1000
    }
    fn fn_00331970_kind_predicate(&mut self, id: u32) -> u32 {
        self.call(0x331970, &[id]) & 1
    }
    fn fn_00331990_placed_count(&mut self, c: u32) -> u32 {
        self.call(0x331990, &[c]) & 3
    }
    fn fn_001531d0_matrix_copy(&mut self, _m: &mut dyn GameplayMemory, destination: u32, source: u32) {
        self.call(0x1531D0, &[destination, source]);
    }
    fn fn_001518f0_matrix_set_translation(&mut self, _m: &mut dyn GameplayMemory, matrix: u32, x: f32, y: f32, z: f32) {
        let _ = z;
        self.call(0x1518F0, &[matrix, x.to_bits(), y.to_bits()]);
    }
    fn fn_00334b20_collect_points(&mut self, m: &mut dyn GameplayMemory, context: u32, cell_low: u32, cell_high: u16, out: u32) -> u32 {
        let found = self.call(0x334B20, &[context, cell_low, u32::from(cell_high)]) & 3;
        for k in 0..4u32 {
            m.write32(out + 4 * k, 0x3E9 + k);
        }
        found
    }
    fn fn_002f7830_flag_bit1(&mut self, id: u32) -> u32 {
        self.call(0x2F7830, &[id]) & 1
    }
    fn fn_002f77e0_flag_bit0(&mut self, id: u32) -> u32 {
        self.call(0x2F77E0, &[id]) & 1
    }
    fn fn_00151e80_transform_point(&mut self, _m: &mut dyn GameplayMemory, matrix: u32, point: u32) {
        self.call(0x151E80, &[matrix, point]);
    }
    fn fn_0009a630_walk_group(&mut self, m: &mut dyn GameplayMemory, start: u32, zero_a: u32, last: u32, records: u32, zero_b: u32, link_count: u32, links: u32) {
        let _ = (zero_a, last, records, zero_b, link_count, links);
        self.call(0x9A630, &[start, 0, last]);
        for k in 0..8u32 {
            m.write8(0x7B67A0 + k, u8::from(k <= start + 1));
        }
    }
}
impl DeserializeHost for Host {
    fn stack_probe(&mut self, size: u32) {
        let _ = size;
        self.call(0x3C9D90, &[]);
    }
    fn find_terminator(&mut self, buffer: u32) {
        self.call(0x32A600, &[buffer]);
    }
    fn reset_selection(&mut self, argument: u32) {
        self.call(0x2FEBF0, &[argument]);
    }
    fn clear_content_tables(&mut self) {
        self.call(0x321C00, &[]);
    }
    fn arena_alloc(&mut self, size: u32, kind: u32) -> u32 {
        self.call(0x3DFA0, &[size, kind]);
        0x0094_0000
    }
    fn arena_release(&mut self, block: u32) {
        self.call(0x3E1E0, &[block]);
    }
    fn finish_load(&mut self) {
        self.call(0x2FE160, &[]);
    }
    fn needs_dirty_flag(&mut self) -> u32 {
        self.call(0x31BDB0, &[])
    }
    fn refresh_a(&mut self) {
        self.call(0x325610, &[]);
    }
    fn refresh_b(&mut self) {
        self.call(0x320F50, &[]);
    }
    fn refresh_c(&mut self) {
        self.call(0x31E160, &[]);
    }
    fn load_preview(&mut self, payload: u32, destination: u32, flag: u32) {
        self.call(0x2FE0F0, &[payload, destination, flag]);
    }
    fn set_packed_entry(&mut self, entry: u32, value: u32) {
        self.call(0x31DD10, &[entry, value]);
    }
    fn name_storage(&mut self) -> u32 {
        self.call(0x3210C0, &[]);
        0x0093_0000
    }
    fn install_grouped(&mut self, table: u32, count: u32) {
        self.call(0x320EA0, &[table, count]);
    }
    fn install_kind3(&mut self, table: u32, size: u32) {
        self.call(0x320FC0, &[table, size]);
    }
    fn map_mode_for(&mut self, map: u32) -> u32 {
        self.call(0x311890, &[map])
    }
    fn setup_preset(&mut self) -> u32 {
        self.call(0x3118F0, &[])
    }
    fn mode_slot_count(&mut self) -> u32 {
        self.call(0x311910, &[])
    }
    fn mode_slots(&mut self) -> u32 {
        self.call(0x311920, &[]);
        0x0095_0000
    }
    fn resolve_point(&mut self, node: u32, destination: u32, value: u32) {
        self.call(0x332330, &[node, destination, value]);
    }
    fn unported_chunk(&mut self, tag: u32) {
        panic!("test buffers must only hold ported chunk kinds, got {tag:#x}");
    }
}
impl StatsHost for Host {
    fn fn_00356870_local_slot(&mut self, player: u32) -> i32 {
        self.call(0x356870, &[player]) as i32
    }
    fn fn_00230530_merge_rule(&mut self, entry: u32) -> u32 {
        let pointer = self.call(0x230530, &[entry]);
        GameplayMemory::read32(&self.view, pointer.wrapping_add(0xC))
    }
    fn fn_001cd170_refresh(&mut self) {
        self.call(0x1CD170, &[]);
    }
}
impl MapmakerSaveHost for Host {
    fn fn_003006c0_serialize(&mut self, buffer: u32, mode: u32) {
        self.call(0x3006C0, &[buffer, mode]);
    }
    fn fn_000827b0_set_mode(&mut self, _m: u32) { unimplemented!() }
    fn selected_slot(&mut self, _m: &dyn GameplayMemory) -> u32 { unimplemented!() }
    fn net_is_live_mode_active(&mut self) -> bool { unimplemented!() }
    fn fn_00025280_load_verify(&mut self, _a: u32, _b: u32, _c: u32, _d: u32, _e: u32, _f: u32) -> u32 { unimplemented!() }
    fn fn_0032a5b0_find_chunk(&mut self, _b: u32, _t: u32) -> u32 { unimplemented!() }
    fn sprintf_label(&mut self, _m: &mut dyn GameplayMemory, _d: u32, _f: u32, _a: u32) { unimplemented!() }
    fn fn_00025670_save_game_delete(&mut self, _m: &mut dyn GameplayMemory, _s: u32, _n: u32) -> u32 { unimplemented!() }
    fn find_enumerate(&mut self, _c: bool, _s: u32, _l: u32, _cb: u32, _x: u32) { unimplemented!() }
    fn fn_000658d0_enter_state(&mut self, _r: u32) { unimplemented!() }
    fn fn_00077470_widget_leaf(&mut self, _w: u32) -> u32 { unimplemented!() }
    fn fn_00337600_refresh(&mut self, _a: u32) { unimplemented!() }
    fn fn_00078980_rebuild_widget(&mut self, _w: u32) { unimplemented!() }
    fn fn_001504f0_crc32(&mut self, _m: &mut dyn GameplayMemory, _b: u32, _l: u32) -> u32 { unimplemented!() }
    fn fn_00025240_signature_size(&mut self, _f: u32) -> u32 { unimplemented!() }
    fn fn_00025710_signature_verify(&mut self, _m: &mut dyn GameplayMemory, _b: u32, _s: u32, _o: u32, _f: u32) -> u32 { unimplemented!() }
    fn fn_00026740_content_write(&mut self, _m: &mut dyn GameplayMemory, _s: u32, _l: u32, _n: u32, _h: u32, _b: u32, _z: u32, _f: u32) -> u32 { unimplemented!() }
    fn fn_000263e0_save_game_write(&mut self, _m: &mut dyn GameplayMemory, _s: u32, _l: u32, _n: u32, _b: u32, _z: u32, _f: u32) -> u32 { unimplemented!() }
}
impl ContentHost for Host {
    fn weapon_set_list_base(&mut self) -> u32 {
        self.call(0x312A60, &[])
    }
}
impl UnlockHost for Host {
    fn fn_00069630_profile_block(&mut self) -> u32 {
        save_profile::profile_ptr(&self.view)
    }
    fn fn_00069900_stats_block(&mut self, index: u32) -> u32 {
        save_profile::stats_record_ptr(&self.view, index)
    }
    fn fn_000696d0_local_slot_active(&mut self) -> u32 {
        let slot = self.call(0x356870, &[0]);
        u32::from(save_profile::slot_is_active(&self.view, slot))
    }
    fn fn_001cceb0_override_byte(&mut self, category: u32) -> u32 {
        i32::from(self.view.byte(0x75AEF8u32.wrapping_add(category)) as i8) as u32
    }
    fn fn_0022e070_record38(&mut self, index: u32) -> u32 {
        self.call(0x22E070, &[index])
    }
    fn fn_0022e050_record_table(&mut self, _i: u32) -> u32 { unimplemented!() }
    fn fn_002be590_find_record30(&mut self, a: u32, b: u32) -> u32 {
        self.call(0x2BE590, &[a, b])
    }
    fn fn_0003d7b0_text_by_id(&mut self, _i: u32) -> u32 { unimplemented!() }
    fn fn_0003fb90_string_copy_case_convert(&mut self, _s: u32) -> u32 { unimplemented!() }
    fn fn_0003d7f0_sprintf(&mut self, _d: u32, _f: u32, _a: u32, _b: u32) -> u32 { unimplemented!() }
    fn fn_00045390_text_context_init(&mut self, _b: u32) { unimplemented!() }
    fn fn_00045820_text_show(&mut self, _c: u32) { unimplemented!() }
    fn fn_00151030_format_text(&mut self, _a: u32, _v: u32) -> u32 { unimplemented!() }
    fn fn_003c8960_widget_set_text(&mut self, _w: u32, _t: u32, _l: u32) { unimplemented!() }
}

struct MCase {
    va: u32,
    args: Vec<u32>,
    regs: [u32; 7],
    script: Vec<[u32; 3]>,
    watch: Vec<(u32, u32)>,
    /// EAX bits that are defined (AL-returning functions leave the upper bytes as residue).
    eax_mask: u32,
    /// T2130: when set, the 0x356870 callee answers this slot on both sides.
    slot: Option<u32>,
    /// T2374 typed cutscene state used by the Tier 1 sequence differential.
    cutscene_sequence: Option<CutsceneSequence>,
}
fn mcase(r: &mut Rng, va: u32, args: Vec<u32>) -> MCase {
    MCase { va, args, regs: std::array::from_fn(|_| r.next()), script: vec![], watch: vec![], eax_mask: u32::MAX, slot: None, cutscene_sequence: None }
}
fn zero(p: &mut Pair, start: u32, len: u32) {
    unsafe {
        std::ptr::write_bytes(p.c.add(start as usize), 0, len as usize);
        std::ptr::write_bytes(p.rust.add(start as usize), 0, len as usize);
    }
}
fn script(r: &mut Rng, rows: usize, special: &[u32]) -> Vec<[u32; 3]> {
    (0..rows).map(|_| [r.word(special), r.next(), r.next()]).collect()
}

fn run_c(p: &mut Pair, c: &MCase) -> (u32, Vec<Row>) {
    p.put32(STACK, 0);
    for (i, a) in c.args.iter().enumerate() {
        p.put32(STACK + 4 + 4 * i as u32, *a);
    }
    let flat: Vec<u32> = c.script.iter().flatten().copied().collect();
    unsafe { sv_script(flat.as_ptr(), c.script.len() as u32) };
    unsafe { sv_set_force_begin(i32::from(FORCE_BEGIN_OK.load(std::sync::atomic::Ordering::Relaxed))) };
    unsafe { sv_set_slot(i32::from(c.slot.is_some()), c.slot.unwrap_or(0)) };
    let mut regs = [c.regs[0], c.regs[1], c.regs[2], c.regs[3], c.regs[4], c.regs[5], c.regs[6], STACK];
    assert_eq!(unsafe { sv_run(c.va, regs.as_mut_ptr()) }, 1, "unknown root {:x}", c.va);
    let trace = (0..unsafe { sv_trace_len() })
        .map(|i| {
            let e = unsafe { *(sv_trace_entry(i) as *const [u32; 16]) };
            // Register-argument callees record their registers instead of stack words.
            match e[0] {
                0x24F40 => [e[0], e[6], 0, e[9]],
                0x250C0 => [e[0], e[6], e[8], 0],
                0x25180 => [e[0], 0, e[8], 0],
                0x25070 | 0x25020 => [e[0], e[10], e[3], 0],
                0x251D0 => [e[0], e[10], 0, 0],
                _ => [e[0], e[3], e[4], e[5]],
            }
        })
        .collect();
    (regs[0], trace)
}

fn mem_diff(p: &Pair, logged: &[u32], watch: &[(u32, u32)]) -> Option<String> {
    let ranges = [(POOL, POOL_LEN), (STACK, ARG_FRAME)];
    let extra = logged.iter().map(|a| (*a, 4)).chain(watch.iter().copied());
    for (start, len) in ranges.iter().copied().chain(extra) {
        for off in 0..len {
            let a = start.wrapping_add(off);
            if p.get(false, a) != p.get(true, a) {
                let c: Vec<u8> = (0..16).map(|k| p.get(false, a.wrapping_add(k))).collect();
                let r: Vec<u8> = (0..16).map(|k| p.get(true, a.wrapping_add(k))).collect();
                return Some(format!("memory {a:08x}: C {c:02x?} Rust {r:02x?}"));
            }
        }
    }
    None
}

/// `port` returns the EAX to compare, or `None` when the original's EAX is undefined residue.
fn drive(
    seed: u64,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> MCase,
    port: impl Fn(&mut Host, &mut View, &MCase) -> Option<u32>,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    FORCE_BEGIN_OK.store(false, std::sync::atomic::Ordering::Relaxed);
    MASK_PADDING.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut r = Rng(seed);
    let (mut with_calls, mut compared) = (0usize, 0usize);
    let mut seen: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        // The lifted body leaves return addresses in the dead frame below ESP, clear it per case.
        zero(p, STACK - 0x2000, 0x2000);
        zero(p, SLOT_TABLE, 0x1F540);
        zero(p, DEVICES, 0x210);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (ceax, mut ctrace) = run_c(p, &c);
        if MASK_PADDING.load(std::sync::atomic::Ordering::Relaxed) {
            for row in ctrace.iter_mut().filter(|row| row[0] == 0x334B20) {
                row[3] &= 0xFFFF;
            }
        }
        let mut view = View(p.rust);
        let mut host = Host { view: View(p.rust), script: c.script.clone(), trace: vec![], slot: c.slot };
        let reax = port(&mut host, &mut view, &c);
        let rtrace = host.trace;
        let dbg_args = c.args.clone();
        let mut why = vec![];
        if let Some(value) = reax {
            compared += 1;
            if (value ^ ceax) & c.eax_mask != 0 {
                why.push(format!("eax C {ceax:#x} Rust {value:#x}"));
            }
        }
        if ctrace != rtrace {
            why.push(format!("trace C {ctrace:x?} Rust {rtrace:x?}"));
        }
        why.extend(mem_diff(p, &logged, &c.watch));
        if !why.is_empty() && c.va == 0x98380 {
            let read32 = |address: u32| {
                u32::from_le_bytes(std::array::from_fn(|i| p.get(false, address + i as u32)))
            };
            let root = c.args[0];
            let node = read32(root + 0x44);
            let count = read32(node + 0x14).min(8);
            let ranges_base = node.wrapping_add(read32(node + 0x10));
            let ranges: Vec<_> = (0..count)
                .map(|i| (read32(ranges_base + i * 8) as i32,
                          read32(ranges_base + i * 8 + 4) as i32))
                .collect();
            eprintln!("cutscene mismatch clock={} start={} rate={} node={node:#x} count={count} ranges={ranges:?} rust_track={:?} rust_query={:?}",
                f32::from_bits(read32(root + 8)), f32::from_bits(read32(root + 0x34)),
                f32::from_bits(read32(0x4d1994)), c.cutscene_sequence.as_ref().and_then(|s| s.track(0)),
                c.cutscene_sequence.as_ref().and_then(|s| s.key_window_active(0)));
        }
        assert!(why.is_empty(), "root {:08X} seed {seed:#x} case {index}: {why:?} args {dbg_args:x?} ctrace {ctrace:x?}", c.va);
        with_calls += usize::from(!ctrace.is_empty());
        for row in &ctrace {
            *seen.entry(row[0]).or_insert(0usize) += 1;
        }
    }
    eprintln!("save t1 {seed:#x}: {SEEDS} cases, {with_calls} with calls, eax compared in {compared}, callee counts {seen:x?}");
}

macro_rules! sv_test {
    ($name:ident, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive($seed, $build, $port);
        }
    };
}

fn big(r: &mut Rng) -> u32 {
    [0u32, 1, 2, 5, 7, 0x10, 0x55, 0x70, 0xE, 0x52, 0x3ED, 0x48F, r.next(), r.below(16)][r.below(14) as usize]
}
/// Random slot table: each of the 16 slots empty or pointing into the pool.
fn seed_slots(r: &mut Rng, p: &mut Pair) {
    for i in 0..16 {
        let value = if r.below(2) == 0 { 0 } else { r.pool(POOL) };
        p.put32(SLOT_TABLE + i * SLOT_STRIDE, value);
    }
}
fn seed_devices(r: &mut Rng, p: &mut Pair) {
    for i in 0..11 {
        let d = DEVICES + i * 0x30;
        p.put8(d, r.below(2) as u8);
        p.put8(d + 3, [0u32, 0x46, 0x55, r.below(256)][r.below(4) as usize] as u8);
        p.put32(d + 4, r.below(8));
        p.put32(d + 0xC, r.next());
        p.put32(d + 8, r.next());
    }
}
const SLOT_WATCH: (u32, u32) = (SLOT_TABLE, 0x1F540);
const DEV_WATCH: (u32, u32) = (DEVICES, 0x210);
const PROFILE_PTR_ADDR: u32 = 0x006F_1E0C;

/// T2379: compare the owned profile/settings update with lifted 0x2CBD20.
/// The original's profile lookup returns a fabricated record address; UI state
/// and enter-state callbacks are intentionally outside this record write-set.
#[test]
fn t2379_owned_profile_record_settings_and_bindings_match_lifted_copy() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(0x2379_1666_1098);
    const RECORD: u32 = POOL + 0x100;
    const VIEWPORT_TABLE: u32 = 0x007B_A364;
    const VIEWPORT: u32 = 0x007B_A9E0;
    const HEAD_SIZE: usize = 0x7C;

    for case in 0..3000 {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        zero(p, STACK - 0x2000, 0x2000);
        p.log.clear();

        // Fabricated profile bytes retain both a nonzero runtime word and
        // arbitrary opaque ranges. Version 2 is the established record form.
        for offset in 0..0x1F54 {
            p.put8(RECORD + offset, r.next() as u8);
        }
        p.put32(RECORD + 0x1EE0, 2);
        let viewport = r.below(4);
        p.put32(VIEWPORT, viewport);
        let source = VIEWPORT_TABLE + viewport * 0x1E0;
        for offset in 0..HEAD_SIZE {
            p.put8(source + offset as u32, r.next() as u8);
        }
        p.put32(0x78AD34, r.below(4));
        p.put32(0x78AD24, r.next());
        p.put32(0x7613E4, r.next());

        let mut c = mcase(&mut r, 0x2CBD20, vec![]);
        c.script = vec![[RECORD + 4, 0, 0]];
        let _ = run_c(p, &c);

        let source_head: [u8; HEAD_SIZE] = std::array::from_fn(|i| p.get(true, source + i as u32));
        let record_bytes: Vec<u8> = (0..0x1F54).map(|offset| p.get(true, RECORD + offset)).collect();
        let mut owned = PlayerRecord::parse(&record_bytes).expect("seeded profile record");
        *owned.settings_mut() = Settings::from_head(&source_head).unwrap();
        for (offset, byte) in owned.encode().into_iter().enumerate() {
            unsafe { *p.rust.add(RECORD as usize + offset) = byte };
        }

        for offset in 0..0x1F54 {
            assert_eq!(
                p.get(false, RECORD + offset),
                p.get(true, RECORD + offset),
                "record byte {offset:#x}, case {case}"
            );
        }
    }
}

// 24900 error map (eax = error, [esp+4] = default)
fn build_24900(r: &mut Rng, _: &mut Pair) -> MCase {
    let default = r.next();
    let mut c = mcase(r, 0x24900, vec![default]);
    c.regs[0] = big(r);
    c
}
sv_test!(t1_00024900, 0x1899_0001, build_24900, |_, _, c| Some(save_device::error_to_status(c.regs[0], c.args[0])));
// 3F890 character filter
fn build_3f890(r: &mut Rng, p: &mut Pair) -> MCase {
    for i in 0..0x81 {
        p.put8(0x4C9040 + i, [0x5Fu32, 0x41, r.below(256)][r.below(3) as usize] as u8);
    }
    let ch = [r.below(0x30), 0x7F + r.below(4), 0x80 + r.below(0x90), r.next() & 0xFFFF, r.next()][r.below(5) as usize];
    mcase(r, 0x3F890, vec![ch])
}
sv_test!(t1_0003f890, 0x1899_0002, build_3f890, |_, v, c| Some(u32::from(save_device::word_char_passes_filter(v, c.args[0] as u16))));

// 31DCD0 flag struct, 322150 node find, 3082D0 list count, small leaves
sv_test!(t1_0031dcd0, 0x1899_0003, |r, p| {
    let rec = r.pool(POOL);
    for off in [4, 8, 0xC, 0x10] {
        p.put32(rec + off, [0u32, 1, r.next()][r.below(3) as usize]);
    }
    mcase(r, 0x31DCD0, vec![rec])
}, |_, v, c| Some(storage::pack_flag_struct(v, c.args[0])));
sv_test!(t1_00322150, 0x1899_0004, |r, p| {
    let state = r.pool(POOL) & !0xFFF;
    p.put32(0x7844A8, state);
    let n = r.below(5);
    p.put32(state + 0x81A4, n);
    let want = r.below(6);
    for i in 0..6 {
        let h = [want, r.next() & 0xFFFF, want | 0x10000][r.below(3) as usize];
        p.put32(state + 0x81A8 + i * 0xB4 + 0xC, h);
    }
    let mut c = mcase(r, 0x322150, vec![want]);
    c.watch.push((state + 0x81A4, 4));
    c
}, |_, v, c| Some(storage::find_node_by_halfword(v, c.args[0] as i32)));
sv_test!(t1_003082d0, 0x1899_0005, |r, p| {
    let head = [0, r.pool(POOL)][r.below(2) as usize];
    let mut at = head;
    for _ in 0..4 {
        if at == 0 { break; }
        p.put32(at, r.below(3));
        at = if r.below(4) == 0 { 0 } else { r.pool(POOL + 0x2000) & !7 };
        p.put32(if head == 0 { 0 } else { head }, p.get(false, head) as u32);
        let _ = at;
        break;
    }
    // chain of three nodes
    let (a, b, d) = (r.pool(POOL), r.pool(POOL + 0x2000), r.pool(POOL + 0x3000));
    for (n, next) in [(a, b), (b, d), (d, 0)] {
        p.put32(n, r.below(3));
        p.put32(n + 0x14, next);
    }
    let key = [r.below(3), 0xFFFF_FFFF][r.below(2) as usize];
    let head0 = if r.below(5) == 0 { 0 } else { a };
    mcase(r, 0x3082D0, vec![head0, key])
}, |_, v, c| Some(storage::count_nodes_with_first_dword(v, c.args[0], c.args[1] as i32)));
sv_test!(t1_0032a3a0, 0x1899_0006, |r, p| { p.put8(0x765A80, r.below(256) as u8); mcase(r, 0x32A3A0, vec![]) },
    |_, v, _| Some(storage::flag_bit_1(v)));
sv_test!(t1_003346e0, 0x1899_0007, |r, p| {
    let a = r.pool(POOL);
    p.put32(a, [0x03E9_03E9u32, 0x03E8_03E9, 0x03E9_03E8, r.next()][r.below(4) as usize]);
    mcase(r, 0x3346E0, vec![a])
}, |_, v, c| Some(u32::from(storage::both_halfwords_at_least_0x3e9(v, c.args[0]))));
sv_test!(t1_00334700, 0x1899_0008, |r, p| {
    let a = r.pool(POOL);
    p.put32(a, r.next());
    p.put32(a + 4, r.next());
    let mut c = mcase(r, 0x334700, vec![a]);
    c.watch.push((a, 8));
    c
}, |_, v, c| { storage::zero_two_words(v, c.args[0]); None });
sv_test!(t1_00312ad0, 0x1899_0009, |r, p| { p.put32(0x784028, r.next()); mcase(r, 0x312AD0, vec![]) },
    |_, v, _| Some(content::weapon_set_preset_index(v)));
sv_test!(t1_00312ae0, 0x1899_000a, |r, _| {
    let (slot, value) = (r.below(8), r.next());
    let mut c = mcase(r, 0x312AE0, vec![slot, value]);
    let base = r.pool(POOL);
    c.script = vec![[base, 0, 0]];
    c.watch.push((POOL, POOL_LEN));
    c
}, |h, v, c| { content::weapon_set_list_set_slot(v, h, c.args[0], c.args[1]); None });

// profile and slot tables
sv_test!(t1_00069630, 0x1899_000b, |r, p| { p.put32(PROFILE_PTR_ADDR, r.next()); mcase(r, 0x69630, vec![]) },
    |_, v, _| Some(save_profile::profile_ptr(v)));
sv_test!(t1_000696d0, 0x1899_000c, |r, p| { seed_slots(r, p); let i = r.below(16); mcase(r, 0x696D0, vec![i]) },
    |_, v, c| Some(u32::from(save_profile::slot_is_active(v, c.args[0]))));
sv_test!(t1_00069720, 0x1899_000d, |r, p| { seed_slots(r, p); let i = [r.below(16), r.below(40) as u32, 0xFFFF_FFFF][r.below(3) as usize]; mcase(r, 0x69720, vec![i]) },
    |_, v, c| Some(save_profile::slot_entry_ptr_plus_4(v, c.args[0] as i32)));
fn field_case(r: &mut Rng, p: &mut Pair, va: u32) -> MCase {
    seed_slots(r, p);
    let i = [r.below(16), r.below(40), 0xFFFF_FFF0u32.wrapping_add(r.below(16))][r.below(3) as usize];
    // Guest address 0 holds a record pointer so null-slot reads are defined.
    p.put32(0, r.pool(POOL + 0x2000));
    for off in [0x24u32, 0x28] {
        let target = u32::from_le_bytes(std::array::from_fn(|b| p.get(false, b as u32)));
        p.put32(target + off, r.next());
    }
    for k in 0..16 {
        for off in [0x24, 0x28] {
            let v = u32::from_le_bytes(std::array::from_fn(|b| p.get(false, SLOT_TABLE + k * SLOT_STRIDE + b as u32)));
            if v != 0 {
                p.put32(v + off, r.next());
            }
        }
    }
    let mut c = mcase(r, va, vec![i]);
    c.watch.push(SLOT_WATCH);
    c.watch.push((POOL, POOL_LEN));
    c
}
sv_test!(t1_00069750, 0x1899_000e, |r, p| field_case(r, p, 0x69750), |_, v, c| Some(save_profile::slot_field_0x24(v, c.args[0] as i32)));
sv_test!(t1_00069790, 0x1899_000f, |r, p| field_case(r, p, 0x69790), |_, v, c| Some(save_profile::slot_field_0x28(v, c.args[0] as i32)));
sv_test!(t1_00069900, 0x1899_0010, |r, p| { seed_slots(r, p); p.put32(0x7DE458, [0u32, 0x80_0000, r.next()][r.below(3) as usize]); let i = r.below(16); mcase(r, 0x69900, vec![i]) },
    |_, v, c| Some(save_profile::stats_record_ptr(v, c.args[0])));
sv_test!(t1_00069930, 0x1899_0011, |r, p| { seed_slots(r, p); p.put32(0x7DE458, [0u32, 0x80_0000, r.next()][r.below(3) as usize]); let (i, e) = (r.below(16), r.below(48)); mcase(r, 0x69930, vec![i, e]) },
    |_, v, c| Some(save_profile::stats_entry_ptr(v, c.args[0], c.args[1])));
sv_test!(t1_000698d0, 0x1899_0012, |r, p| {
    let rec = r.pool(POOL) & !3;
    let tail = r.pool(POOL + 0x2000);
    p.put32(rec, r.below(2));
    p.put32(rec + 0x3C, [0, tail][r.below(2) as usize]);
    p.put32(tail, r.next());
    let arg = if r.below(6) == 0 { 0 } else { rec };
    let mut c = mcase(r, 0x698D0, vec![arg]);
    c.watch.push((POOL, POOL_LEN));
    c
}, |_, v, c| { save_profile::record_zero_and_null_pointer_field(v, c.args[0]); None });
sv_test!(t1_00069640, 0x1899_0013, |r, _| {
    let mut c = mcase(r, 0x69640, vec![]);
    c.script = vec![[r.below(2), 0, 0], [r.below(16), 0, 0]];
    c
}, |h, _, _| Some(save_profile::record_base_for_local_slot_zero(h)));
sv_test!(t1_000696a0, 0x1899_0014, |r, p| {
    p.put32(0x7497B0, r.next());
    let mut c = mcase(r, 0x696A0, vec![]);
    c.script = vec![[r.next(), 0, 0]];
    c
}, |h, v, _| { let s = GameplayMemory::read32(v, 0x7497B0) as i32; Some(save_profile::pack_local_time(s, h)) });
// 697D0 sets fields on an active slot (in-range, non-faulting cases only)
sv_test!(t1_000697d0, 0x1899_0015, |r, p| {
    for i in 0..16 { p.put32(SLOT_TABLE + i * SLOT_STRIDE, [0, r.pool(POOL)][r.below(2) as usize]); }
    p.put32(0, r.pool(POOL + 0x2000));
    let i = [r.below(16), r.below(40)][r.below(2) as usize];
    let (a, b) = (r.next(), r.next());
    let mut c = mcase(r, 0x697D0, vec![i, a, b]);
    c.watch.push((POOL, POOL_LEN));
    c
}, |_, v, c| { save_profile::slot_set_fields(v, c.args[0] as i32, c.args[1], c.args[2]); None });

// device table leaves
sv_test!(t1_00024e40, 0x1899_0016, |r, p| { seed_devices(r, p); let i = r.below(11); mcase(r, 0x24E40, vec![i]) }, |_, v, c| Some(save_device::memory_unit_slot_status(v, c.args[0])));
sv_test!(t1_00024e60, 0x1899_0017, |r, p| { seed_devices(r, p); let i = [r.below(11), r.below(14)][r.below(2) as usize]; { let mut c = mcase(r, 0x24E60, vec![i]); c.eax_mask = 0xFF; c } }, |_, v, c| Some(u32::from(save_device::memory_unit_slot_is_active(v, c.args[0]))));
sv_test!(t1_00024eb0, 0x1899_0018, |r, _| { let i = r.below(11); mcase(r, 0x24EB0, vec![i]) }, |_, _, c| Some(save_device::memory_unit_slot_name_addr(c.args[0])));
sv_test!(t1_00025b80, 0x1899_0019, |r, p| { seed_devices(r, p); let i = [r.below(11), r.below(14)][r.below(2) as usize]; { let mut c = mcase(r, 0x25B80, vec![i]); c.eax_mask = 0xFF; c } }, |_, v, c| Some(u32::from(save_device::memory_unit_slot_drive_letter(v, c.args[0]))));
sv_test!(t1_00025bb0, 0x1899_001a, |r, p| { seed_devices(r, p); let i = [r.below(11), r.below(14)][r.below(2) as usize]; mcase(r, 0x25BB0, vec![i]) }, |_, v, c| Some(save_device::memory_unit_slot_serial(v, c.args[0])));
sv_test!(t1_00025be0, 0x1899_001b, |r, _| {
    let size = [0, r.next(), r.below(0x10000)][r.below(3) as usize];
    let mut c = mcase(r, 0x25BE0, vec![size]);
    c.script = vec![[r.below(0x20000), 0, 0], [r.below(0x20000), 0, 0]];
    c
}, |h, _, c| Some(save_device::save_block_count_for_size(h, c.args[0])));
sv_test!(t1_00025240, 0x1899_001c, |r, _| {
    let flags = r.next();
    let mut c = mcase(r, 0x25240, vec![flags]);
    c.script = vec![[r.next(), 0, 0]];
    c
}, |h, _, c| Some(storage::signature_size_from_flags(c.args[0], h)));
sv_test!(t1_00024d80, 0x1899_001d, |r, p| {
    seed_devices(r, p);
    let i = r.below(11);
    let mut c = mcase(r, 0x24D80, vec![i]);
    let code = big(r);
    c.script = vec![[code, 0, 0]];
    c.watch.push(DEV_WATCH);
    c
}, |h, v, c| { save_device::memory_unit_unmount(v, h, c.args[0]); None });
sv_test!(t1_00024e00, 0x1899_001e, |r, p| {
    seed_devices(r, p);
    let mut c = mcase(r, 0x24E00, vec![]);
    let codes: Vec<u32> = (0..11).map(|_| big(r)).collect();
    c.script = codes.iter().map(|code| [*code, 0, 0]).collect();
    c.watch.push(DEV_WATCH);
    c
}, |h, v, _| { save_device::memory_unit_unmount_all(v, h); None });
sv_test!(t1_000827d0, 0x1899_001f, |r, p| {
    p.put32(0x7B979C, r.next());
    p.put32(0x4D9658, r.below(2));
    let a = [0xFFFF_FFFFu32, r.next(), r.below(11)][r.below(3) as usize];
    let mut c = mcase(r, 0x827D0, vec![a]);
    c.watch.push((0x7B979C, 4));
    c.watch.push((0x4D9658, 4));
    c
}, |_, v, c| { save_device::memory_unit_slot_set(v, c.args[0]); None });
sv_test!(t1_000827f0, 0x1899_0020, |r, p| { p.put32(0x7B979C, r.next()); mcase(r, 0x827F0, vec![]) }, |_, v, _| Some(save_device::memory_unit_slot_get(v)));
sv_test!(t1_000827c0, 0x1899_0021, |r, p| { p.put32(0x7B9798, r.next()); mcase(r, 0x827C0, vec![]) }, |_, v, _| Some(save_device::global_7b9798_get(v)));
sv_test!(t1_00082800, 0x1899_0022, |r, _| { let a = [0xFFFF_FFFFu32, r.below(11)][r.below(2) as usize]; mcase(r, 0x82800, vec![a]) }, |_, _, c| Some(save_device::memory_unit_slot_addr_or_default(c.args[0])));
sv_test!(t1_00082820, 0x1899_0023, |r, p| { p.put32(0x7B979C, [0xFFFF_FFFFu32, r.below(11)][r.below(2) as usize]); mcase(r, 0x82820, vec![]) }, |_, v, _| Some(save_device::memory_unit_slot_addr_or_default_for_selected(v)));
sv_test!(t1_00190690, 0x1899_0024, |r, p| { seed_devices(r, p); p.put32(0x7B979C, r.below(14)); mcase(r, 0x190690, vec![]) }, |_, v, _| Some(save_device::memory_unit_current_slot_status(v)));
sv_test!(t1_00190a70, 0x1899_0025, |r, p| { seed_devices(r, p); p.put32(0x7B979C, r.below(11)); mcase(r, 0x190A70, vec![]) }, |_, v, _| Some(save_device::memory_unit_current_slot_field_0x4(v)));

// unlock helpers (profile block and override bytes seeded; record-38 lookups scripted)
fn seed_profile(r: &mut Rng, p: &mut Pair) {
    let block = POOL + 0x1000;
    p.put32(PROFILE_PTR_ADDR, block);
    for off in (0xBF0..0xC60).step_by(4) {
        p.put32(block + off, [0u32, 0xFFFF_FFFF, r.next()][r.below(3) as usize]);
    }
    for i in 0..8 {
        p.put8(0x75AEF8 + i, [0u32, 1, 0xFF, r.below(256)][r.below(4) as usize] as u8);
    }
    p.put32(0x75AEF4, [0u32, 0, 1][r.below(3) as usize]);
}
sv_test!(t1_001cd2b0, 0x1899_0026, |r, p| {
    seed_profile(r, p);
    let bit = [r.below(0x100) as u32, r.below(64) as u32, 0xFFFF_FFF0u32.wrapping_add(r.below(16))][r.below(3) as usize];
    let diff = [0xFFFF_FFFFu32, r.below(5)][r.below(2) as usize];
    mcase(r, 0x1CD2B0, vec![bit, diff])
}, |h, v, c| Some(save_unlock::profile_bit_set_for_difficulty(v, h, c.args[0] as i32, c.args[1] as i32)));
sv_test!(t1_001cd380, 0x1899_0027, |r, p| {
    seed_profile(r, p);
    let (a, b, c0, d) = (r.below(32), r.below(4), [0u32, 1][r.below(2) as usize], r.below(5));
    mcase(r, 0x1CD380, vec![a, b, c0, d])
}, |h, v, c| Some(save_unlock::profile_bit_set_selector_block(v, h, c.args[0], c.args[1], c.args[2], c.args[3])));
sv_test!(t1_001cd560, 0x1899_0028, |r, p| { seed_profile(r, p); let b = r.below(32); mcase(r, 0x1CD560, vec![b]) },
    |h, v, c| Some(save_unlock::profile_bit_set_word_bf0_or_override(v, h, c.args[0])));
sv_test!(t1_001cd330, 0x1899_0029, |r, p| {
    seed_profile(r, p);
    let rec = r.pool(POOL + 0x2000) & !3;
    p.put32(rec + 4, r.below(5));
    for i in 0..4 { p.put32(rec + 8 + 4 * i, r.below(0x100)); }
    let (a, b) = (r.below(0x20), [0xFFFF_FFFFu32, r.below(5)][r.below(2) as usize]);
    let mut c = mcase(r, 0x1CD330, vec![a, b]);
    c.script = vec![[rec, 0, 0]];
    c
}, |h, v, c| Some(save_unlock::profile_all_bits_set_for_record38(v, h, c.args[0], c.args[1] as i32)));

// profile list lookups and the chunk buffer clear
fn seed_profile_list(r: &mut Rng, p: &mut Pair) -> (u32, u32) {
    let count = r.below(5);
    p.put32(0x74F9FC, count);
    let mut names = vec![];
    for i in 0..6u32 {
        let record = 0x79EE00 + i * 0x150;
        let name: Vec<u8> = (0..r.below(4)).map(|_| 0x61 + r.below(2) as u8).collect();
        for (k, b) in name.iter().chain(std::iter::once(&0)).enumerate() {
            p.put8(record + 0x104 + k as u32, *b);
        }
        p.put32(record + 0x148, r.below(3));
        names.push(name);
    }
    let probe = POOL + 0x200;
    let pick = &names[r.below(6) as usize];
    for (k, b) in pick.iter().chain(std::iter::once(&0)).enumerate() {
        p.put8(probe + k as u32, *b);
    }
    (probe, r.below(3))
}
sv_test!(t1_00190d90, 0x1899_002a, |r, p| {
    let (name, owner) = seed_profile_list(r, p);
    mcase(r, 0x190D90, vec![name, owner])
}, |_, v, c| Some(save_profile::profile_list_name_is_free_for_owner(v, c.args[0], c.args[1])));
sv_test!(t1_00190e00, 0x1899_002b, |r, p| {
    let (name, owner) = seed_profile_list(r, p);
    mcase(r, 0x190E00, vec![name, owner])
}, |_, v, c| Some(save_profile::profile_list_find_entry(v, c.args[0], c.args[1])));
sv_test!(t1_00336640, 0x1899_002c, |r, p| {
    p.put32(0x765C24, 0x0092_0000);
    p.put32(0x765E38, 0x00A0_0000);
    for a in [0x765C20u32, 0x765C28, 0x765C30] { p.put32(a, r.next()); }
    for k in 0..64 {
        p.put32(0x0092_0000 + k * 0x400, r.next());
        p.put32(0x00A0_0000 + k * 0x200, r.next());
    }
    let serialize = r.below(2);
    let mut c = mcase(r, 0x336640, vec![serialize]);
    c.script = vec![[0, 0, 0]];
    c.watch.push((0x0092_0000, 0x10000));
    c.watch.push((0x00A0_0000, 0x8000));
    c.watch.push((0x765C20, 0x14));
    c
}, |h, v, c| { storage::clear_undo_and_level_chunk_buffers(v, h, c.args[0] != 0); None });

// 69A60 stats aggregate (script values double as slot numbers and 230530 descriptor pointers)
sv_test!(t1_00069a60, 0x1899_002d, |r, p| {
    seed_slots(r, p);
    p.put32(0x7DE458, [0u32, 0, 0, 0x80_0000][r.below(4) as usize]);
    for slot in 0..16u32 {
        let block = 0x7C0050 + slot * SLOT_STRIDE;
        for off in (0u32..0xC50).step_by(4) {
            p.put32(block + off, [0u32, 1, r.below(5), r.next()][r.below(4) as usize]);
        }
    }
    for k in 0..24u32 {
        p.put32(k + 0xC, r.below(7));
    }
    p.put32(0xB, r.below(7));
    // The lifted body reads its 5 entry jump table from guest memory.
    for (k, target) in [0x69CC3u32, 0x69CC3, 0x69CC9, 0x69CCF, 0x69CCF].iter().enumerate() {
        p.put32(0x69D2C + 4 * k as u32, *target);
    }
    for off in (0u32..0xC50).step_by(4) {
        p.put32(0x6F11C0 + off, r.next());
    }
    let count = [1u32, 2, 3, 0, 0xFFFF_FFFF][r.below(5) as usize];
    let mut c = mcase(r, 0x69A60, vec![count]);
    c.script = (0..110).map(|_| [[r.below(20), 0xFFFF_FFFF, 0x10][r.below(3) as usize], 0, 0]).collect();
    c.watch.push((0x6F11C0, 0xC50));
    c.watch.push((PROFILE_PTR_ADDR, 4));
    c
}, |h, v, c| { save_profile::profile_stats_aggregate_all_slots(v, h, c.args[0] as i32); None });

// 25E40 / 25FF0 find enumerators (readable ports keep the original frame addresses)
fn seed_find(r: &mut Rng, p: &mut Pair, variant: u32) -> (u32, MCase) {
    seed_devices(r, p);
    p.put8(0x565790, r.below(2) as u8);
    p.put8(0x565793, [0u32, 0x54, r.below(256)][r.below(3) as usize] as u8);
    let (frame_size, name, wide) = if variant == 0 { (0x5c4u32, 0x3c8u32, 0x4ccu32) } else { (0x5ccu32, 0x3d4u32, 0x4d8u32) };
    let base = ((STACK - 4) & !7) - frame_size - 12;
    for k in 0..0x30u32 {
        p.put8(base + name + k, [0u32, 0x41 + r.below(3), 0x2e][r.below(3) as usize] as u8);
        p.put8(base + wide + 2 * k, [0u32, 0x41, 0x7f + r.below(0x90) as u32, r.below(256)][r.below(4) as usize] as u8);
        p.put8(base + wide + 2 * k + 1, [0u32, 0, 0, 1][r.below(4) as usize] as u8);
    }
    for k in 0..0x10u32 {
        p.put8(base + 0x288 + k, r.below(256) as u8);
    }
    let device = [r.below(11), 0xa, r.below(14)][r.below(3) as usize];
    let filter = [0, POOL + 0x100 + r.below(8) * 4][r.below(2) as usize];
    let callback = POOL + 0x300;
    let context = r.next();
    let mut c = mcase(r, if variant == 0 { 0x25E40 } else { 0x25FF0 }, vec![device, filter, callback, context]);
    c.script = (0..64).map(|_| [[0u32, 1, 0xFFFF_FFFF, 5, 0x1000][r.below(5) as usize], 0, 0]).collect();
    c.watch.push((base + 0x0c, 0x4d0));
    (base, c)
}
sv_test!(t1_00025e40, 0x1899_002e, |r, p| seed_find(r, p, 0).1, |h, v, c| {
    save_files::save_game_find_enumerate(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3]);
    None
});
sv_test!(t1_00025ff0, 0x1899_002f, |r, p| seed_find(r, p, 1).1, |h, v, c| {
    save_files::content_find_enumerate(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3]);
    None
});

sv_test!(t1_00026320, 0x1899_0030, |r, p| {
    seed_devices(r, p);
    let args: Vec<u32> = (0..7).map(|k| if k == 6 { [0, POOL + 0x200][r.below(2) as usize] } else { r.next() }).collect();
    let mut c = mcase(r, 0x26320, args);
    c.script = (0..8).map(|_| [r.next(), 0, 0]).collect();
    let base = STACK - 0x120;
    c.watch.push((base, 0x120));
    c
}, |h, v, c| Some(save_files::save_slots_enumerate_for_verify(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3], c.args[4], c.args[5], c.args[6])));

sv_test!(t1_0032b540, 0x1899_0031, |r, p| {
    for k in 0..0x40u32 {
        p.put8(POOL + 0x800 + k, [0u32, 0x41, 0x42][r.below(3) as usize] as u8);
        p.put8(0x765800 + k, [0u32, 0x41][r.below(2) as usize] as u8);
    }
    p.put8(0x47602C, r.below(256) as u8);
    p.put8(0x47602D, r.below(256) as u8);
    p.put32(0x7B979C, [r.below(14), r.next()][r.below(2) as usize]);
    let (context, kind) = (r.next(), [2u32, 2, 2, r.below(6)][r.below(4) as usize]);
    let extra = r.next();
    let mut c = mcase(r, 0x32B540, vec![context, kind, extra]);
    c.script = (0..12).map(|_| [[r.below(20), 8, 7, POOL + 0x800, POOL + 0x810][r.below(5) as usize], 0, 0]).collect();
    c.watch.push((0x765800, 0x100));
    c.watch.push((STACK, 0x10));
    c
}, |h, v, c| { save_files::pad_level_dialog_show_free_space_status(v, h, STACK, c.args[0], c.args[1]); None });

sv_test!(t1_00025710, 0x1899_0032, |r, p| {
    for k in 0..0x40u32 {
        p.put8(POOL + 0x800 + k, r.below(256) as u8);
    }
    let (total, flags) = ([r.below(40), 20 + r.below(40), r.next()][r.below(3) as usize], [0u32, 4, 0x10, 0x14, r.next()][r.below(5) as usize]);
    let out = [0, POOL + 0x700][r.below(2) as usize];
    let buffer = [0, POOL + 0x800][r.below(2) as usize];
    let mut c = mcase(r, 0x25710, vec![buffer, total, out, flags]);
    c.script = (0..12).map(|_| [[1u32, 1, 1, 0, 9, 0xFFFF_FFFF, 20, 5][r.below(8) as usize], 0, 0]).collect();
    c.watch.push((STACK - 0x20, 0x30));
    c.watch.push((POOL + 0x700, 8));
    c
}, |h, v, c| Some(save_files::save_signature_finish(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3])));

sv_test!(t1_000263e0, 0x1899_0033, |r, p| {
    seed_devices(r, p);
    for k in 0..0x20u32 {
        p.put8(POOL + 0x400 + k, [0u32, 0x41, 0x42][r.below(3) as usize] as u8);
    }
    let device = [r.below(11), r.below(14), 0xa][r.below(3) as usize];
    let (label, name) = (POOL + 0x300, POOL + 0x400);
    let data = [0, POOL + 0x800][r.below(2) as usize];
    let length = [0u32, 0x20, r.below(0x100)][r.below(3) as usize];
    let flags = [0u32, 1, 0x10, 0x11, 0xa, 0x24, r.next()][r.below(7) as usize];
    let mut c = mcase(r, 0x263E0, vec![device, label, name, data, length, flags]);
    c.script = (0..40).map(|_| [[0u32, 1, 1, 1, 0xFFFF_FFFF, 5, 0x40, 3][r.below(8) as usize], 0, 0]).collect();
    let base = STACK - 0x448;
    c.watch.push((base + 8, 0x460));
    c.watch.push((DEVICES, 0x210));
    c
}, |h, v, c| {
    Some(save_files::save_game_write(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3], c.args[4], c.args[5]))
});

sv_test!(t1_00025280, 0x1899_0034, |r, p| {
    seed_devices(r, p);
    for k in 0..0x40u32 {
        p.put8(POOL + 0x400 + k, [0u32, 0x41][r.below(2) as usize] as u8);
    }
    let device = [r.below(11), r.below(14)][r.below(2) as usize];
    let data = [0, POOL + 0x800][r.below(2) as usize];
    let length = [0u32, 0x20, r.below(0x100)][r.below(3) as usize];
    let flags = [0u32, 1, 0x10, 0x11, 0xa, 0x24, 4, r.next()][r.below(8) as usize];
    let mut c = mcase(r, 0x25280, vec![device, POOL + 0x400, POOL + 0x300, data, length, flags]);
    c.script = (0..60).map(|_| [[0u32, 1, 1, 1, 0xFFFF_FFFF, 5, 0x40, 0x30, 0x20, 0x90][r.below(10) as usize], 0, 0]).collect();
    let base = STACK - 0x160;
    c.watch.push((base + 0xc, 0x170));
    c.watch.push((DEVICES, 0x210));
    c
}, |h, v, c| Some(save_files::save_file_load_verify(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3], c.args[4], c.args[5])));

sv_test!(t1_00026740, 0x1899_0035, |r, p| {
    // A failing first begin leaves S1.size at u32::MAX and the original then copies 4 GiB (latent bug).
    FORCE_BEGIN_OK.store(true, std::sync::atomic::Ordering::Relaxed);
    seed_devices(r, p);
    for k in 0..0x20u32 {
        p.put8(POOL + 0x400 + k, [0u32, 0x41, 0x42][r.below(3) as usize] as u8);
    }
    let device = [r.below(11), r.below(14), 0xa][r.below(3) as usize];
    let data = [0, POOL + 0x800][r.below(2) as usize];
    let length = [0u32, 0x20, r.below(0x100)][r.below(3) as usize];
    let flags = [0u32, 1, 0x10, 0x11, 0xa, 0x24, r.next()][r.below(7) as usize];
    let hash = r.next();
    let mut c = mcase(r, 0x26740, vec![device, POOL + 0x300, POOL + 0x400, hash, data, length, flags]);
    c.script = (0..100).map(|_| [[0u32, 1, 1, 1, 0xFFFF_FFFF, 5, 0x40, 20, 0x30][r.below(9) as usize], 0, 0]).collect();
    let base = STACK - 0x444;
    c.watch.push((base + 4, 0x440));
    c.watch.push((DEVICES, 0x210));
    c
}, |h, v, c| Some(save_files::content_write(v, h, STACK, c.args[0], c.args[1], c.args[2], c.args[3], c.args[4], c.args[5], c.args[6])));

// 2FA150 tileset context points (frame locals, SSE floats, struct by value to 0x334b20)
fn seed_tileset(r: &mut Rng, p: &mut Pair) -> MCase {
    for id in 0..4u32 {
        let d = 0x529510 + id * 0x140;
        p.put8(d + 0x11, r.below(3) as u8);
        p.put8(d + 0x12, r.below(4) as u8);
    }
    p.put32(0x475D30, 0x3F00_0000);
    p.put32(0x4760CC, 0x3E80_0000);
    let (record, context, obj) = (POOL + 0x500, POOL + 0x600, 0x0090_1100);
    p.put32(record, r.below(4));
    for k in 4..8u32 {
        p.put8(record + k, r.below(256) as u8);
    }
    p.put32(record + 8, [0u32, 0x800_0000, 0x100_0000, 0x900_0000, r.next()][r.below(5) as usize]);
    p.put32(context + 0x3c, POOL + 0x700);
    for n in 0..3u32 {
        let node = POOL + 0x700 + n * 0x40;
        p.put32(node, r.below(5));
        p.put32(node + 0xc, r.next());
        p.put8(node + 0x10, r.below(256) as u8);
        p.put8(node + 0x11, r.below(256) as u8);
        p.put32(node + 0x14, if n < 2 { node + 0x40 } else { 0 });
    }
    p.put32(0x0090_1010, obj);
    let (count, links) = (r.below(5), r.below(4));
    p.put32(obj + 4, count);
    p.put32(obj + 8, links);
    for j in 0..count {
        let rec = obj + 0xc + j * 0x20;
        p.put32(rec, 0x3e9 + r.below(6));
        p.put32(rec + 4, [0u32, 1, 3, 0x41, r.below(256)][r.below(5) as usize]);
        p.put8(rec + 8, r.below(4) as u8);
        p.put8(rec + 9, 0);
        for k in 0xc..0x18u32 {
            p.put8(rec + k, r.below(256) as u8);
        }
    }
    for l in 0..links {
        let at = obj + 0xc + count * 0x20 + l * 0x14;
        p.put32(at, [0u32, 2, 0x10002, r.next()][r.below(4) as usize]);
        p.put32(at + 4, 0x3e9 + r.below(6));
        p.put32(at + 8, [0x3ec, 0x3e9 + r.below(6)][r.below(2) as usize]);
    }
    let mut c = mcase(r, 0x2FA150, vec![record, context]);
    c.script = (0..100).map(|_| [r.below(8), 0, 0]).collect();
    let s = STACK - 0xffc;
    c.watch.push((s + 0x10, 0x6f0));
    c.watch.push((0x7B67A0, 0x20));
    c
}
sv_test!(t1_002fa150, 0x1899_0036, |r, p| seed_tileset(r, p), |h, v, c| Some(content::collect_tileset_context_points(v, h, STACK, c.args[0], c.args[1])));
#[test]
fn padding_mask_is_needed_for_2fa150() {
    let result = std::panic::catch_unwind(|| {
        drive(0x1899_0037, |r, p| { let c = seed_tileset(r, p); MASK_PADDING.store(false, std::sync::atomic::Ordering::Relaxed); c }, |h, v, c| {
            Some(content::collect_tileset_context_points(v, h, STACK, c.args[0], c.args[1]))
        });
    });
    assert!(result.is_err(), "without the padding mask the 0x334b20 argument must diverge");
}

// 301A90 chunk buffer reader, buffers made of the ported chunk kinds only
fn seed_chunks(r: &mut Rng, p: &mut Pair) -> MCase {
    let (editor, buffer) = (0x0092_0000u32, 0x0091_0000u32);
    p.put32(0x7844A8, editor);
    for off in [0x306Eu32, 0x3068, 0x306A, 0x306C, 0x41E0, 0x41D8, 0x41DC, 0xDD10, 0x81A4, 0x280] {
        p.put32(editor + off, r.below(3));
    }
    for k in 0..0x100u32 {
        p.put32(0x0094_0000 + 4 * k, r.next());
    }
    p.put32(0x784020, r.below(3));
    p.put32(0x78402C, r.next());
    p.put32(0x784024, r.next());
    p.put32(0x784028, r.next());
    for off in [0x264u32, 0x254, 0x25C, 0x238, 0x268] {
        p.put32(editor + off, [0u32, 1, 0xFFFF_FFFE, 0xFFFF_FFFF, r.next()][r.below(5) as usize]);
    }
    for k in 0..0x80u32 {
        p.put32(buffer + 0x3000 + 4 * k, r.next());
    }
    // The lifted body reads its 14 entry handler table from guest memory.
    for (k, target) in [0x301C15u32, 0x30217A, 0x302960, 0x302032, 0x302428, 0x30267F, 0x3026B0, 0x30271E, 0x302807, 0x302853, 0x302873, 0x302886, 0x3028DE, 0x30291F].iter().enumerate() {
        p.put32(0x3029DC + 4 * k as u32, *target);
    }
    let good = r.below(8) != 0;
    p.put32(buffer, if good { chunks::BUFFER_MAGIC } else { r.next() });
    p.put32(buffer + 4, if r.below(8) != 0 { chunks::BUFFER_VERSION } else { r.next() });
    let mut at = buffer + 8;
    for _ in 0..r.below(6) {
        let tag = chunks::PORTED_TAGS[r.below(chunks::PORTED_TAGS.len() as u32) as usize];
        let size = match tag {
            chunks::TAG_SETTINGS => 0x3dc,
            chunks::TAG_CHANGE => 0x20,
            chunks::TAG_E4 => 0x80,
            chunks::TAG_B4 => 0xc0,
            chunks::TAG_RECORD_220 => 0x220,
            chunks::TAG_RECORD_8C => 0x90,
            chunks::TAG_PALETTE => 0x100,
            chunks::TAG_PREVIEW => 0x10,
            chunks::TAG_PACKED_FLAGS => 8,
            chunks::TAG_NAMES => 4 * r.below(0x10),
            chunks::TAG_GROUPED => 20 * r.below(5),
            _ => 8 * r.below(8),
        };
        p.put32(at, tag);
        p.put32(at + 4, size);
        for k in 0..size {
            p.put8(at + 8 + k, r.below(256) as u8);
        }
        if tag == chunks::TAG_RECORD_8C {
            p.put32(at + 8 + 0x80, [3u32, 0xa, 0, r.next()][r.below(4) as usize]);
            p.put32(at + 8 + 0x20, r.below(4));
            p.put32(at + 8 + 0x70, r.below(4));
            for k in 0..8u32 {
                p.put32(at + 8 + 0x24 + 4 * k, r.below(40));
                p.put32(at + 8 + 0x28 + 8 * k, r.below(40));
            }
        }
        if tag == chunks::TAG_SETTINGS {
            p.put32(at + 8 + 0x244, [0u32, 1, 0x8000, 0x2_0000, 0xA_8003, r.next()][r.below(6) as usize]);
            p.put8(at + 8 + 0x42, r.below(12) as u8);
            p.put8(at + 8 + 0xa, r.below(3) as u8);
            p.put8(at + 8 + 0xb, 0);
            for k in 0..0x30u32 {
                p.put8(at + 8 + 0x62 + k, [0u32, 0x41, 0x42][r.below(3) as usize] as u8);
                p.put8(at + 8 + 0x82 + k, [0u32, 0x43][r.below(2) as usize] as u8);
                p.put8(at + 8 + 0x162 + k, [0u32, 0x44][r.below(2) as usize] as u8);
            }
        }
        if tag == chunks::TAG_E4 {
            let points = r.below(5);
            p.put8(at + 8 + 0x2c, points as u8);
            p.put8(at + 8 + 0x2d, 0);
        }
        if tag == chunks::TAG_RECORD_220 {
            p.put32(at + 8 + 0x21c, r.below(3));
        }
        at += 8 + size;
    }
    p.put32(at, chunks::TAG_TERMINATOR);
    p.put32(at + 4, 0);
    let flags = r.below(4);
    let mut c = mcase(r, 0x301A90, vec![buffer, flags]);
    c.script = (0..100).map(|_| [r.below(4), 0, 0]).collect();
    let s = ((STACK - 4) & !7) - 0x13f4 - 12;
    c.watch.push((s + 0x10, 0x900));
    c.watch.push((editor, 0x12000));
    c.watch.push((0x783A80, 0x100));
    c.watch.push((0x930000, 0x4b00));
    c.watch.push((0x950000, 0x40));
    c.watch.push((0x783CE0, 0x320));
    c.watch.push((0x784020, 0x10));
    c.watch.push((buffer, 0x3000));
    c
}
sv_test!(t1_00301a90, 0x1899_0038, |r, p| seed_chunks(r, p), |h, v, c| Some(chunks::deserialize_map_state(v, h, STACK, c.args[0], c.args[1])));

#[test]
fn mutant_control_detects_a_perturbed_port() {
    // The harness must be able to fail: a deliberately wrong port (error status off by one).
    let result = std::panic::catch_unwind(|| {
        drive(0x1899_00ff, build_24900, |_, _, c| Some(save_device::error_to_status(c.regs[0], c.args[0]).wrapping_add(1)));
    });
    assert!(result.is_err(), "perturbed port was not detected");
}

// ---- T2130: cutscene state / key cache (tsfp-data-formats) and the save_unlock requirement wrappers ----
//
// Same method as the sections above: identical FABRICATED guest memory in a C arena and a Rust arena,
// the lifted original (EAX where it is a result, nested-call trace, written memory) against the Rust
// semantic API. Register preservation (EBX/ESI/EDI/EBP) and the dead stack are not compared.
use tsfp_data_formats::cutscenes::{CutsceneProp, CutsceneState, KeyNodeProvider};
use tsfp_engine_memory::{GuestMemory as CsGuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

/// Fallible byte memory over the Rust arena (the cutscene API is over `tsfp_engine_memory::GuestMemory`).
struct CsMem(*mut u8);
impl CsGuestMemory for CsMem {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = unsafe { *self.0.add(address.0.wrapping_add(i as u32) as usize) };
        }
        Ok(())
    }
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        for (i, byte) in bytes.iter().enumerate() {
            unsafe { *self.0.add(address.0.wrapping_add(i as u32) as usize) = *byte };
        }
        Ok(())
    }
}
fn put16(p: &mut Pair, address: u32, value: u16) {
    p.log.push(address);
    p.put8(address, value as u8);
    p.put8(address.wrapping_add(1), (value >> 8) as u8);
}
fn cs_state(m: &mut CsMem) -> CutsceneState<'_, CsMem> {
    CutsceneState { memory: m }
}
/// Streaming windows, frame header, offset tables and key cache words used by 0x60390 and 0x60410.
fn cs_seed(r: &mut Rng, p: &mut Pair) {
    let (windows, header, offsets, offsets2) = (POOL + 0x100, POOL + 0x200, POOL + 0x400, POOL + 0x800);
    p.put32(0x7DE530, [0, 0x0091_0000, r.next(), r.below(0x1000)][r.below(4) as usize]);
    p.put32(0x7BB260, [r.below(20), r.below(20), 0, 0xFFFF_FFFF, r.next()][r.below(5) as usize]);
    p.put32(0x7BB284, r.below(8));
    p.put32(0x6D7834, windows);
    for slot in 0..4 {
        p.put32(0x7DE540 + slot * 4, r.below(6));
        p.put32(0x7DE520 + slot * 4, [0u32, 0, 0, 1][r.below(4) as usize]);
    }
    for entry in 0..6 {
        p.put32(windows + entry * 8, (r.below(40) as i32 - 5) as u32);
        p.put32(windows + entry * 8 + 4, r.below(20));
    }
    p.put32(0x6D7818, header);
    p.put32(header + 4, r.below(0x100));
    p.put32(header + 0x14, r.below(0x40));
    p.put32(0x6D7838, offsets);
    for i in 0..64 {
        p.put32(offsets + i * 4, r.below(0x1000));
    }
    p.put32(0x6D783C, offsets2);
    for i in 0..0x200 {
        p.put32(offsets2 + i * 4, r.below(0x1000));
    }
}
fn cs_frame(r: &mut Rng) -> u32 {
    [0, r.below(30), r.below(30), 0u32.wrapping_sub(1 + r.below(3)), r.next()][r.below(5) as usize]
}
sv_test!(t1_00060390, 0x2130_0001, |r, p| { cs_seed(r, p); let f = cs_frame(r); mcase(r, 0x60390, vec![f]) },
    |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).frame_pointer(c.args[0]).unwrap()) });
fn build_60410(r: &mut Rng, p: &mut Pair) -> MCase {
    cs_seed(r, p);
    let (frame, node) = (cs_frame(r), [r.below(8), r.below(8), r.next()][r.below(3) as usize]);
    let near = |r: &mut Rng| [frame, frame.wrapping_add(1), frame.wrapping_sub(1), 0u32.wrapping_sub(1000), r.below(30)][r.below(5) as usize];
    p.put32(0x4D19CC, if r.below(2) == 0 { node } else { r.below(8) });
    let (first, second) = (near(r), near(r));
    p.put32(0x4D19D0, first);
    p.put32(0x4D19D4, second);
    p.put32(0x6D782C, r.next());
    p.put32(0x6D7830, r.next());
    mcase(r, 0x60410, vec![frame, node])
}
sv_test!(t1_00060410, 0x2130_0002, build_60410,
    |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).key_node(c.args[0] as i32, c.args[1]).unwrap().0) });
fn seed_entry_rows(r: &mut Rng, p: &mut Pair) {
    for word in 0..(13 * 0x44 / 4) {
        p.put32(0x4D1AC0 + word * 4, r.next());
    }
    for row in 0..13 {
        put16(p, 0x4D1AC6 + row * 0x44, [r.below(6), 0xFFFF, r.below(4) + 1][r.below(3) as usize] as u16);
    }
}
fn entry_key(r: &mut Rng) -> u32 {
    [r.below(6), 0xFFFF_FFFF, r.below(5), r.next()][r.below(4) as usize]
}
sv_test!(t1_00064d90, 0x2130_0003, |r, p| { seed_entry_rows(r, p); let k = entry_key(r); mcase(r, 0x64D90, vec![k]) },
    |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).entry_pointer(c.args[0] as i32).unwrap().0) });
sv_test!(t1_00064dd0, 0x2130_0004, |r, p| { seed_entry_rows(r, p); let (a, k) = ([0, 1, r.next(), r.below(3)][r.below(4) as usize], entry_key(r)); mcase(r, 0x64DD0, vec![a, k]) },
    |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).entry_word(c.args[0], c.args[1] as i32).unwrap()) });
sv_test!(t1_00058240, 0x2130_0005, |r, p| {
    let index = [r.below(64), r.below(64), 0xFFFF_FFFF, r.next()][r.below(4) as usize];
    let mapped = [r.below(20) as u16, 0xFFFF, 0xFFFE, 0x8000, 0x7FFF, r.next() as u16][r.below(6) as usize];
    put16(p, 0x7E03C0u32.wrapping_add(index.wrapping_mul(2)), mapped);
    let entry = 0x7DE660u32.wrapping_add((mapped as i16 as i32 as u32).wrapping_mul(8));
    p.put32(entry, r.next());
    p.put32(entry + 4, r.next());
    let (a, b) = (r.next(), r.next());
    mcase(r, 0x58240, vec![index, a, b])
}, |_, v, c| { let mut m = CsMem(v.0); cs_state(&mut m).set_track_pair(c.args[0], c.args[1], c.args[2]).unwrap(); None });
fn seed_prop(r: &mut Rng, p: &mut Pair) -> u32 {
    let prop = POOL + 0x1000 + r.below(8) * 0x400;
    for word in 0..0x80 {
        p.put32(prop + word * 4, r.next());
    }
    prop
}
sv_test!(t1_0008e970, 0x2130_0006, |r, p| { let prop = seed_prop(r, p); mcase(r, 0x8E970, vec![prop]) },
    |_, v, c| { let mut m = CsMem(v.0); cs_state(&mut m).reset_prop(CutsceneProp(GuestAddr(c.args[0]))).unwrap(); None });
fn build_94220(r: &mut Rng, p: &mut Pair) -> MCase {
    let prop = seed_prop(r, p);
    let record = POOL + 0x3000 + r.below(8) * 0x40;
    for word in 0..0x10 {
        p.put32(record + word * 4, r.next());
    }
    p.put32(prop, record);
    let args = vec![prop, r.next(), r.next(), r.next(), r.next()];
    mcase(r, 0x94220, args)
}
fn port_94220(v: &mut View, c: &MCase, swap: bool) {
    let mut m = CsMem(v.0);
    let (second, third) = if swap { (c.args[4] as u8, c.args[3] as u8) } else { (c.args[3] as u8, c.args[4] as u8) };
    cs_state(&mut m).set_prop_state(CutsceneProp(GuestAddr(c.args[0])), c.args[1] as u8, c.args[2], second, third).unwrap();
}
sv_test!(t1_00094220, 0x2130_0007, build_94220, |_, v, c| { port_94220(v, c, false); None });
sv_test!(t1_00098da0, 0x2130_0008, |r, p| { p.put32(0x70E164, [0, r.next(), r.below(16)][r.below(3) as usize]); mcase(r, 0x98DA0, vec![]) },
    |_, v, _| { let mut m = CsMem(v.0); Some(cs_state(&mut m).active_record().unwrap().0) });
sv_test!(t1_00098db0, 0x2130_0009, |r, p| {
    let record = if r.below(3) == 0 { 0 } else { POOL + 0x3000 + r.below(8) * 0x40 };
    p.put32(0x70E164, record);
    for word in 0..0x10 {
        p.put32(POOL + 0x3000 + word * 4, r.next());
    }
    p.put32(record.wrapping_add(0x1C), r.next());
    p.put32(0x4D977C, r.next());
    mcase(r, 0x98DB0, vec![])
}, |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).request_stop().unwrap(); None });
sv_test!(t1_00061680, 0x2130_000a, |r, p| {
    for word in 0..(0x40 / 4) {
        p.put32(0x6D7840 + word * 4, r.next());
    }
    for a in [0x6D785C, 0x6D7860, 0x6D7864, 0x6D7868, 0x6D786C, 0x6D7870, 0x6D7874, 0x6D7878] {
        p.put32(a, r.next());
    }
    mcase(r, 0x61680, vec![])
}, |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).reset_stream_state().unwrap(); None });
sv_test!(t1_0006ee00, 0x2130_000b, |r, p| { p.put32(0x7BB248, r.next()); mcase(r, 0x6EE00, vec![]) },
    |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).invalidate_subtitle().unwrap(); None });
sv_test!(t1_0023efc0, 0x2130_000c, |r, p| {
    for a in [0x78CD58, 0x75CE58, 0x78CD54, 0x75CE5C] {
        p.put32(a, r.next());
    }
    let mut c = mcase(r, 0x23EFC0, vec![]);
    c.watch.push((0x78CD54, 8));
    c
}, |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).reset_sequence_selection().unwrap(); None });
sv_test!(t1_0006f4b0, 0x2130_000d, |r, p| {
    p.put32(0x6F73D0, r.next());
    p.put32(0x6F73D4, r.next());
    let (a, b) = (r.next(), r.next());
    mcase(r, 0x6F4B0, vec![a, b])
}, |_, v, c| { let mut m = CsMem(v.0); cs_state(&mut m).set_slot_pair(c.args[0], c.args[1]).unwrap(); None });
sv_test!(t1_0003ee20, 0x2130_000e, |r, p| { p.put32(0x6B83E0, r.next()); p.put32(0x6B83E4, r.next()); mcase(r, 0x3EE20, vec![]) },
    |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).clear_stream_pair().unwrap(); None });

sv_test!(t1_000665b0, 0x2130_000f, |r, p| { p.put32(0x6F11BC, r.next()); mcase(r, 0x665B0, vec![]) },
    |_, v, _| { let mut m = CsMem(v.0); cs_state(&mut m).clear_sequence_gate().unwrap(); None });
sv_test!(t1_0006f430, 0x2130_0010, |r, p| {
    for word in 0..(20 * 36 / 4) {
        p.put32(0x51FC48 + word * 4, r.next());
    }
    let index = [r.below(20), r.below(20), 0u32.wrapping_sub(r.below(3)), 20 + r.below(3), r.next()][r.below(5) as usize];
    mcase(r, 0x6F430, vec![index])
}, |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).slot_word(c.args[0] as i32).unwrap()) });

// ---- T2374: Rust-owned cutscene sequence/key-window control against lifted 0x98380 ----

const CUTSCENE_ROOT: u32 = POOL + 0x100;
const CUTSCENE_NODE: u32 = POOL + 0x1200;
const CUTSCENE_RANGES: u32 = POOL + 0x1400;
const CUTSCENE_RATE: u32 = 0x004D_1994;

fn t2374_definition() -> CutsceneDefinition {
    static DEFINITION: OnceLock<CutsceneDefinition> = OnceLock::new();
    DEFINITION
        .get_or_init(|| {
            let csinfo = tsfp_format_csinfo::Csinfo::parse(b"FRAMES 1").unwrap();
            let mut bytes = vec![0; 256];
            bytes[..4].copy_from_slice(b"ANRS");
            for (offset, value) in [
                (4, 200), (12, 1), (16, 1), (20, 1), (24, 1), (28, 200),
                (32, 208), (36, 212),
            ] {
                bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
            }
            let animation = tsfp_format_igcs::Animation::parse(&bytes).unwrap();
            CutsceneDefinition::new(csinfo, animation)
        })
        .clone()
}

fn finite_cutscene_time(r: &mut Rng) -> f32 {
    ((r.below(601) as i32 - 100) as f32) / 10.0
}

fn build_cutscene_sequence(r: &mut Rng, p: &mut Pair) -> MCase {
    let now = finite_cutscene_time(r);
    let start = finite_cutscene_time(r);
    let rate = [0.0f32, 0.5, 1.0, 10.0, 30.0, -1.0][r.below(6) as usize];
    let current_frame = ((now - start) * rate).trunc() as i32;
    let count = r.below(5) as usize;

    p.put32(CUTSCENE_ROOT + 8, now.to_bits());
    p.put32(CUTSCENE_ROOT + 0x34, start.to_bits());
    p.put32(CUTSCENE_ROOT + 0x44, CUTSCENE_NODE);
    // 0x98380 reads inline key records at node + the byte offset in +0x10;
    // +0x14 is the key count. Keep the table outside descriptor words while
    // preserving the relative-offset contract.
    p.put32(CUTSCENE_NODE + 0x10, CUTSCENE_RANGES - CUTSCENE_NODE);
    p.put32(CUTSCENE_NODE + 0x14, count as u32);
    p.put32(CUTSCENE_RATE, rate.to_bits());

    let mut key_ranges = Vec::with_capacity(count);
    for index in 0..count {
        let first = current_frame.wrapping_add(r.below(21) as i32 - 10);
        let last = first.wrapping_add(r.below(16) as i32);
        key_ranges.push(KeyFrameRange { first, last });
    }
    key_ranges.sort_by_key(|range| range.last);
    for (index, range) in key_ranges.iter().enumerate() {
        p.put32(CUTSCENE_RANGES + index as u32 * 8, range.first as u32);
        p.put32(CUTSCENE_RANGES + index as u32 * 8 + 4, range.last as u32);
    }

    let mut sequence = CutsceneSequence::new(t2374_definition(), 1, rate);
    let mut track = PlaybackTrack::new(0, 0, start);
    track.enabled = true;
    track.key_ranges = key_ranges;
    sequence.set_track(0, track).unwrap();
    sequence.seek(now);
    let mut case = mcase(r, 0x98380, vec![CUTSCENE_ROOT, 0]);
    case.cutscene_sequence = Some(sequence);
    case
}

sv_test!(t1_00098380_sequence_key_window, 0x2374_0001, build_cutscene_sequence,
    |_, _, c| Some(c.cutscene_sequence.as_ref().unwrap().key_window_active(0).unwrap() as u32));

#[test]
fn cutscene_mutant_controls_detect_perturbed_ports() {
    // Each deliberately wrong port must fail the same comparison (return pointer, written bytes, argument order).
    let key_node = std::panic::catch_unwind(|| {
        drive(0x2130_00f1, build_60410, |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).key_node(c.args[0] as i32, c.args[1]).unwrap().0.wrapping_add(4)) });
    });
    assert!(key_node.is_err(), "perturbed key_node was not detected");
    let swapped = std::panic::catch_unwind(|| {
        drive(0x2130_00f2, build_94220, |_, v, c| { port_94220(v, c, true); None });
    });
    assert!(swapped.is_err(), "swapped prop state bytes were not detected");
    // The streaming-window path of 0x60390 and the +C/+E word choice of 0x64DD0 must both be reachable.
    let windows = std::panic::catch_unwind(|| {
        drive(0x2130_00f3, |r, p| { cs_seed(r, p); let f = cs_frame(r); mcase(r, 0x60390, vec![f]) },
            |_, v, _| { let mut m = CsMem(v.0); Some(cs_state(&mut m).frame_pointer(0).unwrap()) });
    });
    assert!(windows.is_err(), "ignoring the frame argument of frame_pointer was not detected");
    let alternate = std::panic::catch_unwind(|| {
        drive(0x2130_00f4, |r, p| { seed_entry_rows(r, p); let (a, k) = ([0, 1, r.next(), r.below(3)][r.below(4) as usize], entry_key(r)); mcase(r, 0x64DD0, vec![a, k]) },
            |_, v, c| { let mut m = CsMem(v.0); Some(cs_state(&mut m).entry_word(u32::from(c.args[0] == 0), c.args[1] as i32).unwrap()) });
    });
    assert!(alternate.is_err(), "inverted +C/+E choice of entry_word was not detected");
}

// unlock requirement wrappers: tables, records and lists are FABRICATED small-valued words.
const UNLOCK_RECORDS: u32 = POOL + 0x2800;
const UNLOCK_LISTS: u32 = POOL + 0x3000;
fn put_descriptor(r: &mut Rng, p: &mut Pair, at: u32) {
    p.put32(at, r.below(10));
    p.put32(at + 4, [r.below(16), r.below(40), r.next()][r.below(3) as usize]);
    p.put32(at + 8, [r.below(5), 0xFFFF_FFFF, r.below(4)][r.below(3) as usize]);
    p.put32(at + 0xC, if r.below(3) == 0 { 0 } else { UNLOCK_LISTS + r.below(4) * 0x20 });
    p.put32(at + 0x10, r.below(6));
}
/// Profile block, override bytes, descriptor tables, the 8 records (+0x14 and +0x24 hold descriptors), the lists.
fn seed_unlock(r: &mut Rng, p: &mut Pair) -> MCase {
    seed_profile(r, p);
    seed_slots(r, p);
    // The 0x1CD530 type jump table (docs/t-save-layout.md, "Unlock descriptor switch"; checked against the
    // retail image once, locally). The harness arena holds no code, so the nine handler addresses are seeded.
    for (kind, target) in [0x1CD528u32, 0x1CD4DC, 0x1CD43B, 0x1CD423, 0x1CD469, 0x1CD455, 0x1CD47D, 0x1CD4E5, 0x1CD4A7].into_iter().enumerate() {
        p.put32(0x1CD530 + 4 * kind as u32, target);
    }
    let slot = r.below(4);
    if r.below(4) != 0 {
        p.put32(SLOT_TABLE + slot * SLOT_STRIDE, POOL + 0x100);
    }
    for entry in 0..13 {
        put_descriptor(r, p, save_unlock::TABLE_4FCC88 + entry * 0x1C);
    }
    for base in [save_unlock::TABLE_4FA144, save_unlock::TABLE_4D23AC] {
        for entry in 0..8 {
            put_descriptor(r, p, base + entry * 0x3C);
        }
    }
    for record in 0..8 {
        let at = UNLOCK_RECORDS + record * 0x40;
        for word in 0..16 {
            p.put32(at + word * 4, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0xFFFF_FFFF][r.below(11) as usize]);
        }
        p.put32(at + 4, r.below(5));
        put_descriptor(r, p, at + 0x14);
        put_descriptor(r, p, at + 0x24);
    }
    for word in 0..32 {
        let end = r.below(5) == 0;
        p.put32(UNLOCK_LISTS + word * 4, if end || word % 8 == 7 { 0xFFFF_FFFF } else { r.below(10) });
    }
    let mut c = mcase(r, 0, vec![]);
    c.slot = Some(slot);
    // Record pointers (0x22E070, 0x2BE590 results) in call order; the slot callee ignores its row.
    c.script = (0..128).map(|_| [if r.below(8) == 0 { 0 } else { UNLOCK_RECORDS + r.below(8) * 0x40 }, 0, 0]).collect();
    c
}
fn unlock_case(r: &mut Rng, p: &mut Pair, va: u32, args: impl FnOnce(&mut Rng) -> Vec<u32>) -> MCase {
    let mut c = seed_unlock(r, p);
    c.va = va;
    c.args = args(r);
    c
}
fn table_index(r: &mut Rng, count: u32) -> u32 {
    [r.below(count), r.below(count), r.below(count + 3), r.next()][r.below(4) as usize]
}
sv_test!(t1_001cd3e0, 0x2130_0101, |r, p| unlock_case(r, p, 0x1CD3E0, |r| vec![UNLOCK_RECORDS + r.below(8) * 0x40 + [0x14, 0x24][r.below(2) as usize], r.next()]),
    |h, v, c| Some(save_unlock::unlock_requirement_met(v, h, c.args[0])));
sv_test!(t1_001cd940, 0x2130_0102, |r, p| unlock_case(r, p, 0x1CD940, |r| vec![table_index(r, 13)]),
    |h, v, c| Some(save_unlock::requirement_met_table_4fcc88(v, h, c.args[0])));
sv_test!(t1_001cd040, 0x2130_0103, |r, p| unlock_case(r, p, 0x1CD040, |_| vec![]),
    |h, v, _| Some(save_unlock::unlock_bitmask_of_met_entries(v, h)));
sv_test!(t1_001cd980, 0x2130_0104, |r, p| unlock_case(r, p, 0x1CD980, |r| vec![r.below(8), r.below(8)]),
    |h, v, c| Some(save_unlock::requirement_met_record30_payload(v, h, c.args[0], c.args[1])));
sv_test!(t1_002be5e0, 0x2130_0105, |r, p| unlock_case(r, p, 0x2BE5E0, |r| vec![r.next(), r.below(8)]),
    |h, _, c| Some(save_unlock::find_record30_payload(h, c.args[0], c.args[1])));
sv_test!(t1_001cd9d0, 0x2130_0106, |r, p| unlock_case(r, p, 0x1CD9D0, |r| vec![table_index(r, 8)]),
    |h, v, c| Some(save_unlock::requirement_met_weapon_slot(v, h, c.args[0])));
sv_test!(t1_001cd9f0, 0x2130_0107, |r, p| unlock_case(r, p, 0x1CD9F0, |r| vec![table_index(r, 8)]),
    |h, v, c| Some(save_unlock::requirement_met_table_4d23ac(v, h, c.args[0])));
sv_test!(t1_001cda20, 0x2130_0108, |r, p| unlock_case(r, p, 0x1CDA20, |r| vec![[r.below(14), r.below(8), r.next()][r.below(3) as usize]]),
    |h, v, c| Some(save_unlock::requirement_met_record38_entry(v, h, c.args[0])));
sv_test!(t1_001cd890, 0x2130_0109, |r, p| {
    let mut c = unlock_case(r, p, 0x1CD890, |r| vec![[0, 1, r.below(14), r.next()][r.below(4) as usize], r.next(), r.next()]);
    c.watch.push((STACK - 0x14, 0x14));
    c
}, |h, v, c| Some(save_unlock::unlock_requirement_met_type2_override_1(v, h, c.args[0], c.args[2], STACK - 0x14)));

#[test]
fn unlock_mutant_controls_detect_perturbed_ports() {
    // Skipping the override-byte check of 0x1CD9F0 must change EAX whenever the override is set.
    let skip_override = std::panic::catch_unwind(|| {
        drive(0x2130_01f1, |r, p| unlock_case(r, p, 0x1CD9F0, |r| vec![table_index(r, 8)]), |h, v, c| {
            Some(save_unlock::unlock_requirement_met(v, h, save_unlock::TABLE_4D23AC.wrapping_add(c.args[0].wrapping_mul(0x3C))))
        });
    });
    assert!(skip_override.is_err(), "missing override check was not detected");
    // A wrong host argument (category 4 instead of 5 is not reachable here, so perturb the key order).
    let swapped_keys = std::panic::catch_unwind(|| {
        drive(0x2130_01f2, |r, p| unlock_case(r, p, 0x2BE5E0, |r| vec![r.next(), r.below(8)]), |h, _, c| {
            Some(save_unlock::find_record30_payload(h, c.args[1], c.args[0]))
        });
    });
    assert!(swapped_keys.is_err(), "swapped record-30 keys were not detected");
}
