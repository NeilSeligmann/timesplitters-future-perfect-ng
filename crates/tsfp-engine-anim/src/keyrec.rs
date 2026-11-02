//! Key record and layer init helpers (T1668, rule 10 batch 3): `0x606E0`, `0x60740`, `0x949C0`, `0x8EA20`, `0x585B0`.
//! Tier 1 via `tests/keyrec_golden.rs` (seeded worlds against the original under Unicorn).
use crate::start::Layer;

/// `0x606E0(frame)` for the in-memory key store `[0x6D780C]` (`store` = its dwords): the byte offset of the 64 byte key record of `frame`
/// clamped into `0..count` (`count = [0x7BB260]`). The store header word 2 is a byte offset (rounded toward zero to a dword multiple) to
/// a table: `[group index][16 x cell index]`, the record is `table + 4 + cell * 64`. With the stream flag `[0x7BB288]` the original returns
/// `0x60410(frame, 0)` instead ([`KeyRecord::Streamed`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRecord {
    Store(u32),
    Streamed(u32),
}

pub fn clamp_frame(frame: i32, count: i32) -> i32 {
    let frame = frame.max(0);
    if frame >= count { count - 1 } else { frame }
}

pub fn key_record(store: &[u32], count: i32, frame: i32, streamed: Option<&dyn Fn(i32) -> u32>) -> KeyRecord {
    let frame = clamp_frame(frame, count);
    if let Some(stream) = streamed {
        return KeyRecord::Streamed(stream(frame));
    }
    let table = (store[2] as i32 / 4) as usize;
    let cell = store[table + 1 + (store[table] as usize) * 16 + frame as usize];
    KeyRecord::Store((4 * table as u32 + 4).wrapping_add(cell.wrapping_mul(64)))
}

/// `0x60740` field copies of a key record (dwords): `+0..0xC` (4 dwords), `+0x10..0x18` (3), `+0x1C` (1).
pub fn key_fields(record: &[u32]) -> ([u32; 4], [u32; 3], u32) {
    ([record[0], record[1], record[2], record[3]], [record[4], record[5], record[6]], record[7])
}

/// `0x949C0(records)`: byte size of a list of 10 byte records (payload length in byte 2, a record whose byte 3 is negative ends the list),
/// each payload rounded up to 4 bytes except the last one.
pub fn records_byte_size(bytes: &[u8]) -> u32 {
    let negative = |record: usize| bytes[10 * record + 3] as i8 >= 0;
    if !negative(0) {
        return 0;
    }
    let (mut total, mut record) = (0u32, 0usize);
    loop {
        let length = u32::from(bytes[10 * record + 2]);
        let next_continues = negative(record + 1);
        total += if next_continues { (length + 3) & !3 } else { length };
        record += 1;
        if !next_continues {
            return total;
        }
    }
}

/// `0x8EA20(a, b, id, speed)`: restarts layer `a` for animation `id` (slot from `slot_of`, speed, times and root cells cleared) and clears the
/// same cells of layer `b`; only these fields are written.
pub fn init_layers(a: &mut Layer, b: &mut Layer, id: u32, slot: u32, speed: f32) {
    a.0[0] = id;
    a.0[1] = slot;
    for i in [2, 3, 10, 11, 12, 4, 5, 6, 7, 8, 9] {
        a.0[i] = 0;
    }
    a.0[13] = speed.to_bits();
    a.0[21] = 0;
    a.0[20] = 0;
    for i in [21, 10, 11, 12, 4, 5, 6, 7, 8, 9, 20] {
        b.0[i] = 0;
    }
}

/// Sample scale of a clip query: the shape scale (`[obj+0x7C]+0xB08`) for objects of type 8, else 1.0 (also for a null object).
pub fn query_scale(object_type: Option<u32>, shape_scale: f32) -> f32 {
    if object_type == Some(8) { shape_scale } else { 1.0 }
}

/// `0x59A40(obj, id, out3)`: root displacement of a whole clip, `position(last key) - position(key 0)` of node 0 (positions already scaled).
pub fn root_displacement(first: [f32; 3], last: [f32; 3]) -> [f32; 3] {
    [last[0] - first[0], last[1] - first[1], last[2] - first[2]]
}

/// `0x59B00(obj, out_first, out_last, out_frames, axis)`: root coordinate `axis` (0 x, 1 y, 2 z) at key 0 and at the last key plus the clip's frame
/// count as float (`clip+0xC`); other axes only report the frame count (`None` for the coordinates).
pub fn axis_range(first: [f32; 3], last: [f32; 3], axis: u32, frames: i32) -> (Option<(f32, f32)>, f32) {
    let range = (axis < 3).then(|| (first[axis as usize], last[axis as usize]));
    (range, frames as f32)
}
