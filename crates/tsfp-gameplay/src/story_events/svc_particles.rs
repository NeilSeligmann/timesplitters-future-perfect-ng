//! T1856 engine services of the particle and sound request family (original addresses in the doc
//! `docs/t-story-event-particles-t1856.md`).
//!
//! * `0x129E0` emitter vertex fetch, `0x12B840` particle ring spawn (`0x157CC0` small random),
//! * `0x10E2B0` object region refresh over `0x873C0` / `0x87310` / `0x157440` / `0x1571C0` / `0x47560` / `0x87260`,
//! * the request wrappers `0x217B0`, `0x218A0`, `0x21860` (record from `0x45390`, submitted by `0x45820`).
//!
//! All functions work on [`GuestMemory`] with the original guest addresses so they can be compared with the
//! oracle word for word. The request submit `0x45820` stays behind [`EffectSink`]. MEASURED against Unicorn.

// Negated ordered compares are deliberate: they reproduce the x86 `comiss`/`ja`/`jb` NaN behaviour of the original.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::prelude_1e2b80::init_spawn_record;
use super::strip_update::{GuestMemory, Service, StripServices};

/// `[0x7450D0]`: next slot of the particle ring.
pub const PARTICLE_INDEX: u32 = 0x7450D0;
/// Base of the particle ring (200 slots of `0x38` bytes).
pub const PARTICLE_RING: u32 = 0x74_2510;
/// Slot count of the particle ring.
pub const PARTICLE_SLOTS: u32 = 0xC8;
/// `[0x7497CC]`: state of the small LCG `0x153BB0`.
pub const SMALL_RNG: u32 = 0x74_97CC;
/// `[0x708788]`: region table (entries of `0xB4` bytes).
pub const REGION_TABLE: u32 = 0x70_8788;
/// Root of the region tree.
pub const REGION_TREE: u32 = 0x70_2E00;

const F_1_1024: f32 = f32::from_bits(0x3A80_0000);
const F_ONE_BITS: u32 = 0x3F80_0000;
const F_PARTICLE_B: u32 = 0x3C23_D70A;
const F_DEFAULT_RADIUS: f32 = f32::from_bits(0x4080_0000);
const F_NODE_SCALE: f32 = f32::from_bits(0x3880_0200);

fn f(mem: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(mem.read_u32(address))
}

fn u16_at(mem: &dyn GuestMemory, address: u32) -> u16 {
    u16::from(mem.read_u8(address)) | u16::from(mem.read_u8(address.wrapping_add(1))) << 8
}

/// `0x153BB0`: `state = state * 0x343FD + 0x269EC3` (32 bit), returns `(state >> 16) & 0x7FFF`.
pub fn small_rng(mem: &mut dyn GuestMemory) -> u32 {
    let state = mem
        .read_u32(SMALL_RNG)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x26_9EC3);
    mem.write_u32(SMALL_RNG, state);
    (state >> 16) & 0x7FFF
}

/// `0x157CC0`: `small_rng() & 0x1F`.
pub fn small_rng_5bit(mem: &mut dyn GuestMemory) -> u32 {
    small_rng(mem) & 0x1F
}

/// `0x129E0(out4, table, entry, record, pick)` (cdecl): fills four words at `out` with the emitter vertex
/// `[entry + 4] + (u16 [record + 6] + pick) * stride`. When bit `0x10` of `[table + 0xC]` is set the vertices are
/// packed (stride 8, three i16 scaled by 1/1024 plus the base at `table + 0x38..0x40`, fourth word is the i16 at
/// `+6` ORed with `0x3F800000`), else stride 16 and the four words are copied. MEASURED.
pub fn emitter_offset(
    mem: &mut dyn GuestMemory,
    out: u32,
    table: u32,
    entry: u32,
    record: u32,
    pick: u32,
) {
    let index = u32::from(u16_at(mem, record.wrapping_add(6))).wrapping_add(pick);
    let base = mem.read_u32(entry.wrapping_add(4));
    if mem.read_u8(table.wrapping_add(0xC)) & 0x10 != 0 {
        let vertex = base.wrapping_add(index.wrapping_mul(8));
        for axis in 0..3u32 {
            let raw = u16_at(mem, vertex.wrapping_add(2 * axis)) as i16;
            let value = f32::from(raw) * F_1_1024 + f(mem, table.wrapping_add(0x38 + 4 * axis));
            mem.write_u32(out.wrapping_add(4 * axis), value.to_bits());
        }
        let w = i32::from(u16_at(mem, vertex.wrapping_add(6)) as i16) as u32;
        mem.write_u32(out.wrapping_add(12), w | F_ONE_BITS);
    } else {
        let vertex = base.wrapping_add(index << 4);
        let words: [u32; 4] =
            std::array::from_fn(|i| mem.read_u32(vertex.wrapping_add(4 * i as u32)));
        for (i, word) in words.iter().enumerate() {
            mem.write_u32(out.wrapping_add(4 * i as u32), *word);
        }
    }
}

/// `0x12B840(pos3, dir3, scale)` (cdecl, scale passed as a float stack word): writes the next slot of the
/// 200 entry particle ring: `+0 = 3`, `+4 = small_rng & 0x1F`, `+8..+0x10` position, `+0x14..+0x1C` direction,
/// `+0x20 = +0x24 = scale`, `+0x28 = 0.01`, `+0x2C = 1.0`, `+0x34 = 0x103000`, then advances `[0x7450D0]` (wraps
/// to 0 at 200). INFERRED meaning (a billboard particle), layout MEASURED.
pub fn spawn_particle(mem: &mut dyn GuestMemory, pos: u32, dir: u32, scale_bits: u32) {
    let index = mem.read_u32(PARTICLE_INDEX);
    let slot = index.wrapping_mul(0x38).wrapping_add(PARTICLE_RING);
    mem.write_u32(slot, 3);
    let seed = small_rng_5bit(mem);
    mem.write_u32(slot + 4, seed);
    mem.write_u32(slot + 0x2C, F_ONE_BITS);
    mem.write_u32(slot + 0x20, scale_bits);
    mem.write_u32(slot + 0x28, F_PARTICLE_B);
    mem.write_u32(slot + 0x34, 0x10_3000);
    mem.write_u32(slot + 0x24, scale_bits);
    for k in 0..3u32 {
        let value = mem.read_u32(pos.wrapping_add(4 * k));
        mem.write_u32(slot + 8 + 4 * k, value);
    }
    for k in 0..3u32 {
        let value = mem.read_u32(dir.wrapping_add(4 * k));
        mem.write_u32(slot + 0x14 + 4 * k, value);
    }
    // Re-read: slot 200 (index 200) is the index cell itself, so the first store of the slot changed it.
    let next = (mem.read_u32(PARTICLE_INDEX) as i32).wrapping_add(1);
    mem.write_u32(
        PARTICLE_INDEX,
        if next < PARTICLE_SLOTS as i32 {
            next as u32
        } else {
            0
        },
    );
}

// ---------------------------------------------------------------------------------------------------------
// Region tree query: 0x873C0 / 0x87310 / 0x157440 / 0x1571C0 / 0x47560 / 0x87260
// ---------------------------------------------------------------------------------------------------------

/// `0x47560(a, b, c, d)` (four vec3 pointers): 1 when box `[a, b]` overlaps `[c, d]`, i.e. no axis has `a > d` or
/// `c > b` (comparisons are ordered `>` so NaN never rejects). MEASURED.
pub fn boxes_overlap(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) -> bool {
    (0..3).all(|i| !(a[i] > d[i]) && !(c[i] > b[i]))
}

fn vec3(mem: &dyn GuestMemory, address: u32) -> [f32; 3] {
    std::array::from_fn(|i| f(mem, address.wrapping_add(4 * i as u32)))
}

/// `0x87260(qmin, qmax, out, count, list)` leaf callback: for each u16 region id in `list` whose entry
/// (`[0x708788] + id * 0xB4`) has bit `0x10` of the byte `+0xB2` clear, sets the bit and appends the u16
/// `id * 2 | overlap(query, entry box +0x10..+0x1C..+0x28)` to `out`. Returns 0.
fn leaf_callback(
    mem: &mut dyn GuestMemory,
    qmin: [f32; 3],
    qmax: [f32; 3],
    out: &mut Vec<u16>,
    count: u32,
    list: u32,
) {
    for k in 0..count {
        let id = u32::from(u16_at(mem, list.wrapping_add(2 * k)));
        let table = mem.read_u32(REGION_TABLE);
        let entry = table.wrapping_add(id.wrapping_mul(0xB4));
        let flags = mem.read_u8(entry.wrapping_add(0xB2));
        if flags & 0x10 == 0 {
            mem.write_u8(entry.wrapping_add(0xB2), flags | 0x10);
            let overlap = boxes_overlap(
                qmin,
                qmax,
                vec3(mem, entry.wrapping_add(0x10)),
                vec3(mem, entry.wrapping_add(0x1C)),
            );
            out.push((id * 2) as u16 | u16::from(overlap));
        }
    }
}

/// `0x1571C0(tree, qmin, qmax, node, nmin, nmax, callback, out)`: recursive walk of the packed k-d tree. Node
/// words live at `[tree]` (u16 array); a node `(w & 0xC000) == 0xC000` is a leaf whose list is at
/// `[tree + 4] + (w & 0x3FFF) * 2` (u16 count, then ids); otherwise axis `w >> 14` and split
/// `(nmax - nmin) * ((w & 0x3FFF) * 6.1038881e-5) + nmin` (f32 steps), the low child (`2n`, max clipped to the
/// split) is visited when `split >= qmin[axis]`, the high child (`2n + 1`, min raised to the split) when
/// `qmax[axis] >= split` (unordered compares skip the child). A child returning 1 stops the walk; the leaf callback
/// returns 0, so the walk always returns 0. Depth is capped at 64 (the original overflows its stack on a cyclic
/// tree, which the node doubling cannot produce).
#[allow(clippy::too_many_arguments)]
fn tree_walk(
    mem: &mut dyn GuestMemory,
    tree: u32,
    qmin: [f32; 3],
    qmax: [f32; 3],
    node: u32,
    nmin: [f32; 3],
    nmax: [f32; 3],
    out: &mut Vec<u16>,
    depth: u32,
) -> u32 {
    if depth > 64 {
        return 0;
    }
    let nodes = mem.read_u32(tree);
    let word = u32::from(u16_at(mem, nodes.wrapping_add(node.wrapping_mul(2))));
    if word & 0xC000 == 0xC000 {
        let list = mem
            .read_u32(tree.wrapping_add(4))
            .wrapping_add((word & 0x3FFF) * 2);
        let count = u32::from(u16_at(mem, list));
        leaf_callback(mem, qmin, qmax, out, count, list.wrapping_add(2));
        return 0;
    }
    let axis = (word >> 14) as usize;
    let fraction = ((word & 0x3FFF) as i32 as f32) * F_NODE_SCALE;
    let split = (nmax[axis] - nmin[axis]) * fraction + nmin[axis];
    let mut result = 0;
    if !(split < qmin[axis]) {
        let mut clipped = nmax;
        clipped[axis] = split;
        result = tree_walk(
            mem,
            tree,
            qmin,
            qmax,
            node.wrapping_mul(2),
            nmin,
            clipped,
            out,
            depth + 1,
        );
        if result == 1 {
            return 1;
        }
    }
    if !(qmax[axis] < split) {
        let mut raised = nmin;
        raised[axis] = split;
        result = tree_walk(
            mem,
            tree,
            qmin,
            qmax,
            node.wrapping_mul(2).wrapping_add(1),
            raised,
            nmax,
            out,
            depth + 1,
        );
    }
    result
}

/// `0x157440(tree, qmin, qmax, callback, out)`: when the query box overlaps the root box (`+8` min, `+0x14` max)
/// walks the tree from node 1.
fn tree_query(
    mem: &mut dyn GuestMemory,
    tree: u32,
    qmin: [f32; 3],
    qmax: [f32; 3],
    out: &mut Vec<u16>,
) {
    let root_min = vec3(mem, tree.wrapping_add(8));
    let root_max = vec3(mem, tree.wrapping_add(0x14));
    if boxes_overlap(qmin, qmax, root_min, root_max) {
        tree_walk(mem, tree, qmin, qmax, 1, root_min, root_max, out, 0);
    }
}

/// `0x87310(box, ring, ring_len)`: queries the tree `0x702E00` with the six float box at `box` (min, max) and
/// clears bit `0x10` of every hit's flag byte. Hit words with bit 0 set (the box overlaps) store `id` into the
/// dword ring until `ring_len` are stored. Returns the stored count. The original scratch buffer holds `0xD6`
/// hits; a bigger result would corrupt its stack and is not modelled. MEASURED.
pub fn region_query(mem: &mut dyn GuestMemory, query: u32, ring: &mut [u32]) -> u32 {
    let qmin = vec3(mem, query);
    let qmax = vec3(mem, query.wrapping_add(0xC));
    let mut hits = Vec::new();
    tree_query(mem, REGION_TREE, qmin, qmax, &mut hits);
    let table = mem.read_u32(REGION_TABLE);
    let mut stored = 0usize;
    let mut store = true;
    for hit in hits {
        let entry = table
            .wrapping_add(u32::from(hit >> 1).wrapping_mul(0xB4))
            .wrapping_add(0xB2);
        let flags = mem.read_u8(entry);
        mem.write_u8(entry, flags & 0xEF);
        if store && hit & 1 != 0 {
            if let Some(slot) = ring.get_mut(stored) {
                *slot = u32::from(hit >> 1);
            }
            stored += 1;
            if stored == ring.len() {
                store = false;
            }
        }
    }
    stored as u32
}

/// `0x873C0(_, box, ring, ring_len)`: 0 when the table `[0x708788]` is null, else [`region_query`].
pub fn region_query_checked(mem: &mut dyn GuestMemory, query: u32, ring: &mut [u32]) -> u32 {
    if mem.read_u32(REGION_TABLE) == 0 {
        0
    } else {
        region_query(mem, query, ring)
    }
}

/// `0x10E2B0` (register `esi` = object): refreshes the region id list `[obj + 0xE0]` (count) and bytes
/// `[obj + 0xE4 ..]`. With a definition `[obj + 0x88]` and not (flags `[obj + 4]` has `0x20000` and `0x10000000`)
/// the count is the signed byte `[def + 0x30]` and the bytes are `[def + 0x38 + 4 * i]`. Otherwise the box
/// `[obj + 0xC8..0xE0]` is built from the six floats at `+0xB0..+0xC8` plus the position `+0x14` (min corner
/// minus, max corner plus the radius, `[obj + 0xAC]` when flag `0x20000` is set else 4.0) and the region query
/// fills the list (at most 12 entries, low bytes copied). MEASURED.
pub fn refresh_regions(mem: &mut dyn GuestMemory, object: u32) {
    let def = mem.read_u32(object + 0x88);
    let flags = mem.read_u32(object + 4);
    let at = |k: u32| object.wrapping_add(k);
    if def != 0 && !(flags & 0x2_0000 != 0 && flags & 0x1000_0000 != 0) {
        let count = i32::from(mem.read_u8(def + 0x30) as i8);
        mem.write_u32(at(0xE0), count as u32);
        for i in 0..count.max(0) as u32 {
            let byte = mem.read_u8(def + 0x38 + 4 * i);
            mem.write_u8(at(0xE4 + i), byte);
        }
        return;
    }
    let radius = if flags & 0x2_0000 != 0 {
        f(mem, at(0xAC))
    } else {
        F_DEFAULT_RADIUS
    };
    let lows = [(0xB0, 0x14), (0xB4, 0x18), (0xB8, 0x1C)];
    let highs = [(0xBC, 0x14), (0xC0, 0x18), (0xC4, 0x1C)];
    for (k, (base, pos)) in lows.iter().enumerate() {
        let value = f(mem, at(*base)) + f(mem, at(*pos)) - radius;
        mem.write_u32(at(0xC8 + 4 * k as u32), value.to_bits());
    }
    for (k, (base, pos)) in highs.iter().enumerate() {
        let value = f(mem, at(*base)) + f(mem, at(*pos)) + radius;
        mem.write_u32(at(0xD4 + 4 * k as u32), value.to_bits());
    }
    let mut ring = [0u32; 12];
    let count = region_query_checked(mem, at(0xC8), &mut ring);
    mem.write_u32(at(0xE0), count);
    for (i, word) in ring.iter().take((count as i32).max(0) as usize).enumerate() {
        mem.write_u8(at(0xE4 + i as u32), *word as u8);
    }
}

// ---------------------------------------------------------------------------------------------------------
// Request wrappers 0x217B0 / 0x218A0 / 0x21860
// ---------------------------------------------------------------------------------------------------------

/// `0x45820(&record)`: submit of a 12 word request (ported by `svc_effect_submit`). The 12 words are the contract,
/// the return value is the handle the wrappers return.
pub trait EffectSink {
    fn submit(&mut self, record: &[u32; 12]) -> u32;
}

/// `0x217B0(id, pos3)`: record from `0x45390`, `[0] = id`, position words 7..9, byte `+0x28 = 1`, submit.
pub fn start_effect(
    mem: &dyn GuestMemory,
    sink: &mut dyn EffectSink,
    id: u32,
    position: u32,
) -> u32 {
    let mut record = init_spawn_record();
    record[0] = id;
    for k in 0..3u32 {
        record[7 + k as usize] = mem.read_u32(position.wrapping_add(4 * k));
    }
    record[10] = (record[10] & !0xFF) | 1;
    sink.submit(&record)
}

/// `0x218A0(id, value)`: record from `0x45390`, `[0] = id`, `[6] = value` (float bits, replaces 1.0), submit.
pub fn sound_1(sink: &mut dyn EffectSink, id: u32, value_bits: u32) -> u32 {
    let mut record = init_spawn_record();
    record[0] = id;
    record[6] = value_bits;
    sink.submit(&record)
}

/// `0x21860(id, count, value)`: record from `0x45390`, `[0] = id`, `[1] = count`, `[6] = value`, submit.
pub fn sound_2(sink: &mut dyn EffectSink, id: u32, count: u32, value_bits: u32) -> u32 {
    let mut record = init_spawn_record();
    record[0] = id;
    record[1] = count;
    record[6] = value_bits;
    sink.submit(&record)
}

/// [`StripServices`] adapter: handles `StartEffect` (`0x217B0`), `EmitterOffset` (`0x129E0`) and `SpawnParticle`
/// (`0x12B840`) with the ports above and forwards every other service to `inner`.
pub struct ParticleStripServices<'a> {
    pub sink: &'a mut dyn EffectSink,
    pub inner: &'a mut dyn StripServices,
}

impl StripServices for ParticleStripServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
        match service {
            Service::StartEffect => start_effect(memory, self.sink, args[0], args[1]),
            Service::EmitterOffset => {
                emitter_offset(memory, args[0], args[1], args[2], args[3], args[4]);
                0
            }
            Service::SpawnParticle => {
                spawn_particle(memory, args[0], args[1], args[2]);
                0
            }
            other => self.inner.call(memory, other, args),
        }
    }
}
