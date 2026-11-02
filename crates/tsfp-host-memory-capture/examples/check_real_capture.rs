// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline comparison of a real original-code host-memory capture (T3928, memory scope: calls, results, read bytes).
//! This fixture is not a game recording or a retirement gate.
use std::{
    collections::{BTreeMap, VecDeque},
    env, fs,
};
use tsfp_engine_memory::guest_regions::{GuestRegions, Placement, RegionRequest, Usage};
use tsfp_host_memory_capture::{events, read};

fn u32_at(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(b[n..n + 8].try_into().unwrap())
}
#[derive(Clone, Debug)]
struct Place {
    kind: u32,
    length: u64,
    address: u32,
    result: u32,
}
const MAX_MAPPED: usize = 3 << 30;
fn insert_map(maps: &mut BTreeMap<u32, Vec<u8>>, base: u32, length: u64) -> bool {
    let Ok(length) = usize::try_from(length) else {
        return false;
    };
    let Some(end) = (base as u64).checked_add(length as u64) else {
        return false;
    };
    if length == 0
        || length > MAX_MAPPED
        || end > (1u64 << 32)
        || maps
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(length)
            > MAX_MAPPED
        || maps.iter().any(|(&other, bytes)| {
            (base as u64) < u64::from(other) + bytes.len() as u64 && u64::from(other) < end
        })
    {
        return false;
    }
    maps.insert(base, vec![0; length]);
    true
}
#[derive(Default)]
struct Replay {
    maps: BTreeMap<u32, Vec<u8>>,
    expected: VecDeque<Place>,
    error: bool,
}
impl Replay {
    fn range(&self, addr: u32, len: usize) -> Option<(u32, usize)> {
        self.maps
            .range(..=addr)
            .next_back()
            .and_then(|(&base, data)| {
                let offset = (addr - base) as usize;
                (offset.checked_add(len)? <= data.len()).then_some((base, offset))
            })
    }
}
impl Placement for Replay {
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32> {
        let Some(event) = self.expected.pop_front() else {
            self.error = true;
            return None;
        };
        if event.kind != 0 || event.length != length || event.address != fixed.unwrap_or(0) {
            self.error = true;
            return None;
        }
        if event.result == 0 {
            return None;
        }
        if !insert_map(&mut self.maps, event.result, length) {
            self.error = true;
            return None;
        }
        Some(event.result)
    }
    fn unmap(&mut self, base: u32, length: u64) {
        let event = self.expected.pop_front();
        if !event.is_some_and(|e| e.kind == 1 && e.length == length && e.address == base)
            || self.maps.remove(&base).is_none()
        {
            self.error = true;
        }
    }
    fn backs_guest(&self) -> bool {
        true
    }
    fn read_guest(&self, addr: u32, out: &mut [u8]) -> bool {
        let Some((base, offset)) = self.range(addr, out.len()) else {
            return false;
        };
        out.copy_from_slice(&self.maps[&base][offset..offset + out.len()]);
        true
    }
    fn write_guest(&mut self, addr: u32, data: &[u8]) {
        let Some((base, offset)) = self.range(addr, data.len()) else {
            self.error = true;
            return;
        };
        self.maps.get_mut(&base).unwrap()[offset..offset + data.len()].copy_from_slice(data);
    }
}
fn main() {
    let path = env::args_os().nth(1).expect("capture path");
    let data = fs::read(path).expect("capture file");
    let decoded = read(data.as_slice(), 4 << 30).expect("sealed stream");
    let ops = [
        (12, 4, 0),
        (14, 1, 0),
        (15, 2, 0),
        (16, 2, 0),
        (13, 1, 0),
        (1, 9, 1),
        (2, 1, 0),
        (3, 3, 4),
        (4, 2, 4),
        (5, 2, 4),
        (6, 0, 10),
        (7, 0, 2),
        (8, 0, 2),
        (9, 0, 2),
        (10, 3, 0),
        (20, 1, 0),
        (21, 1, 2),
        (22, 2, 1),
        (23, 3, 1),
        (24, 3, 0),
        (25, 1, 0),
        (26, 0, 0),
        (27, 1, 11),
        (28, 1, 11),
    ];
    let calls = events::validate(&decoded, b"tsfp-original-c-host", &ops).expect("valid stream");
    println!("stream: {} records, {calls} calls", decoded.len());
    let mut counts: BTreeMap<u32, u64> = BTreeMap::new();
    let mut rust = GuestRegions::new(Replay::default());
    let mut current: Option<(u32, Vec<u32>, VecDeque<Place>)> = None;
    let mut nested_ops = Vec::new();
    for (kind, data) in decoded.iter().skip(2) {
        match *kind {
            events::CALL_BEGIN => {
                let op = u32_at(data, 24);
                if op != 90 {
                    if current.is_some() {
                        nested_ops.push(op);
                        continue;
                    }
                    *counts.entry(op).or_default() += 1;
                    let count = u32_at(data, 28) as usize;
                    current = Some((
                        op,
                        (0..count).map(|n| u32_at(data, 32 + n * 4)).collect(),
                        VecDeque::new(),
                    ));
                }
            }
            events::PLACEMENT => {
                let e = Place {
                    kind: u32_at(data, 8),
                    length: u64_at(data, 12),
                    address: u32_at(data, 20),
                    result: u32_at(data, 24),
                };
                current
                    .as_mut()
                    .expect("placement within heap call")
                    .2
                    .push_back(e);
            }
            events::MEMORY_READ | events::MEMORY_WRITE => {}
            events::CALL_END if current.is_some() => {
                if let Some(op) = nested_ops.pop() {
                    let _ = op;
                    continue;
                }
                let (op, args, expected) = current.take().unwrap();
                rust.placement_mut().expected = expected;
                let result = match op {
                    27 | 28 => {
                        let region = if op == 27 {
                            rust.region_at(args[0])
                        } else {
                            rust.region_containing(args[0])
                        };
                        let words = region.map_or([0; 11], |r| {
                            [
                                r.address,
                                r.size,
                                r.requested,
                                r.physical,
                                r.protect,
                                r.alloc_protect,
                                r.state,
                                u32::from(r.contiguous),
                                u32::from(r.persist),
                                r.generation as u32,
                                (r.generation >> 32) as u32,
                            ]
                        });
                        for (i, word) in words.iter().enumerate() {
                            assert_eq!(*word, u32_at(data, 28 + i * 4), "region query field {i}");
                        }
                        u32::from(region.is_some())
                    }
                    26 => {
                        rust.reset();
                        0
                    }
                    24 => rust.heap_realloc(args[0], args[1], args[2]),
                    25 => 0,
                    14 => rust.heap_valid(args[0]) as u32,
                    23 => {
                        let size = if args[2] != 0 {
                            rust.heap_block_size(args[0], args[1])
                        } else {
                            None
                        };
                        assert_eq!(size.unwrap_or(0), u32_at(data, 28));
                        u32::from(size.is_some())
                    }
                    20 => rust.physical_address(args[0]),
                    21 => {
                        assert_eq!(rust.allocation_generation(args[0]), u64_at(data, 28));
                        0
                    }
                    22 => {
                        let mapped = if args[1] != 0 {
                            rust.virtual_from_physical(args[0])
                        } else {
                            None
                        };
                        assert_eq!(mapped.unwrap_or(0), u32_at(data, 28));
                        u32::from(mapped.is_some())
                    }
                    10 => {
                        let usage = match args[2] {
                            0 => Usage::Virtual,
                            1 => Usage::Pool,
                            2 => Usage::Stack,
                            3 => Usage::Cache,
                            _ => panic!("unsupported usage"),
                        };
                        rust.region_set_usage(args[0], args[1], usage) as u32
                    }
                    7..=9 => {
                        let value = match op {
                            7 => rust.region_count() as u64,
                            8 => rust.heap_count() as u64,
                            9 => rust.mapped_bytes(),
                            _ => unreachable!(),
                        };
                        assert_eq!(value, u64_at(data, 28), "registry count operation {op}");
                        0
                    }
                    6 => {
                        let s = rust.statistics();
                        let values = [
                            s.virtual_committed_bytes,
                            s.virtual_reserved_bytes,
                            s.pool_pages,
                            s.stack_pages,
                            s.cache_pages,
                        ];
                        for (i, expected) in values.iter().enumerate() {
                            assert_eq!(*expected, u64_at(data, 28 + i * 8), "statistics field {i}");
                        }
                        0
                    }
                    1 => {
                        assert_eq!(args[8], 0, "fixture usage must be virtual");
                        let request = RegionRequest {
                            bytes: args[0],
                            alignment: args[1],
                            lowest_physical: args[2],
                            highest_physical: args[3],
                            protect: args[4],
                            state: args[5],
                            contiguous: args[6] != 0,
                            fixed_base: args[7],
                        };
                        let allocation = rust.region_alloc_typed(&request, Usage::Virtual);
                        let (address, status) = match allocation {
                            Ok(address) => (address, 0),
                            Err(status) => (0, status),
                        };
                        assert_eq!(status, u32_at(data, 28));
                        address
                    }
                    2 => rust.region_free(args[0]) as u32,
                    3 => rust.region_set_protect(args[0], args[1], args[2]) as u32,
                    4 => rust.region_set_persist(args[0], args[1] != 0) as u32,
                    5 => rust.region_set_state(args[0], args[1]) as u32,
                    12 => rust.heap_create_typed(args[0], args[1], args[2], Usage::Virtual),
                    15 => rust.heap_alloc(args[0], args[1]),
                    16 => rust.heap_free(args[0], args[1]) as u32,
                    13 => rust.heap_destroy(args[0]) as u32,
                    _ => panic!("unsupported operation {op}"),
                };
                assert_eq!(
                    result,
                    u32_at(data, 8),
                    "operation {op} result (call {})",
                    counts.values().sum::<u64>()
                );
                if [3, 4, 5].contains(&op) {
                    let region = if op == 3 {
                        rust.region_containing(args[0])
                    } else {
                        rust.region_at(args[0])
                    };
                    let state =
                        region.map_or([0; 4], |r| [1, r.protect, r.state, u32::from(r.persist)]);
                    for (i, value) in state.iter().enumerate() {
                        assert_eq!(*value, u32_at(data, 28 + i * 4), "region state after {op}");
                    }
                }
                let placement = rust.placement_mut();
                assert!(
                    !placement.error && placement.expected.is_empty(),
                    "placement mismatch after operation {op}"
                );
            }
            _ => {}
        }
    }
    assert!(current.is_none() && nested_ops.is_empty());
    println!(
        "T3928: Rust registry matched every captured call result and placement; ops by id: {counts:?}"
    );
    println!(
        "INFERRED scope: guest writes and heap bytes are not captured; no Tier 2 claim beyond calls/results/placements"
    );
}
