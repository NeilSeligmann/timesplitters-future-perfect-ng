// SPDX-License-Identifier: GPL-3.0-or-later
//! T1663 Tier 1 differential against the retained lifted request bodies.
//! Inputs are FABRICATED guest states; this is not an original-instruction or runtime proof.

use std::collections::BTreeMap;

use tsfp_engine_fileio::{
    asset_requests::{
        AssetEpoch, AssetRequestHost, AssetRequestInput, AssetRequests, DestinationLease,
        LegacySlotSnapshot, REQUEST_SLOT_COUNT,
    },
    async_io::{CompletionHost, Volume},
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    assets::{self, AssetGuest},
    memory::GuestMemory,
};

const SLOT_BASE: u32 = 0x0066_1370;
const SLOT_STRIDE: u32 = 0x44;
const SENTINEL: u32 = 0x0066_4890;
const STACK: u32 = 0x9000;

#[derive(Default, Clone)]
struct Memory(BTreeMap<u32, u8>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(index as u32), byte);
        }
    }
    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes([
            self.read8(address),
            self.read8(address.wrapping_add(1)),
            self.read8(address.wrapping_add(2)),
            self.read8(address.wrapping_add(3)),
        ])
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }
}

#[derive(Default)]
struct Guest;

impl GuestCall for Guest {
    fn call(&mut self, address: u32, _: &mut ExtRegisters) {
        panic!("unexpected lifted callee {address:#x}");
    }
}

impl AssetGuest for Guest {
    fn writeback_invalidate(&mut self, _: u32) {}
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

#[derive(Default)]
struct Host {
    now: u64,
    writes: Vec<(DestinationLease, Vec<u8>)>,
    closed: Vec<u32>,
    signals: Vec<u32>,
}

impl CompletionHost for Host {
    fn now(&self) -> u64 {
        self.now
    }
    fn advance_to(&mut self, ticks: u64) {
        self.now = self.now.max(ticks);
    }
    fn identity(&self, file: u32) -> u32 {
        file
    }
    fn identity_live(&self, _: u32) -> bool {
        false
    }
    fn write_buffer(&mut self, _: u32, _: &[u8]) -> bool {
        false
    }
    fn write_iosb(&mut self, _: u32, _: u32, _: u32) {}
    fn signal_event(&mut self, event: u32) {
        self.signals.push(event);
    }
    fn event_signalled(&self, event: u32) -> bool {
        self.signals.contains(&event)
    }
}

impl AssetRequestHost for Host {
    fn write_asset(&mut self, destination: DestinationLease, bytes: &[u8]) -> bool {
        self.writes.push((destination, bytes.to_vec()));
        true
    }
    fn close_asset_file(&mut self, handle: u32) {
        self.closed.push(handle);
    }
}

fn slot_address(slot: usize) -> u32 {
    SLOT_BASE + slot as u32 * SLOT_STRIDE
}

fn project_slot(memory: &mut Memory, slot: usize, entry: LegacySlotSnapshot) {
    let at = slot_address(slot);
    memory.put32(at + 0x08, entry.file_offset);
    memory.put32(at + 0x0c, entry.reserved_0c);
    memory.put32(at + 0x10, entry.reserved_10);
    memory.put32(at + 0x14, entry.file_handle);
    memory.put32(at + 0x18, entry.file_base);
    memory.put8(at + 0x20, u8::from(entry.close_file));
    memory.put32(at + 0x24, entry.request_word);
    memory.put32(at + 0x2c, entry.aligned_size);
    memory.put32(at + 0x30, entry.timestamp_delta);
    memory.put32(at + 0x34, entry.state);
    memory.put32(at + 0x38, entry.destination);
    memory.put32(at + 0x3c, entry.previous);
    memory.put32(at + 0x40, entry.next);
}

fn project_globals(memory: &mut Memory, requests: &AssetRequests) {
    memory.put32(SENTINEL + 0x3c, requests.legacy_tail());
    memory.put32(SENTINEL + 0x40, requests.legacy_head());
    memory.put32(
        0x0066_48d4,
        requests
            .legacy_selected()
            .map_or(0, |id| slot_address(usize::from(id.slot))),
    );
    memory.put32(0x0066_48d8, requests.legacy_active_count());
}

fn project_request_slots(memory: &mut Memory, requests: &AssetRequests, slots: &[usize]) {
    for &slot in slots {
        project_slot(memory, slot, requests.legacy_slot(slot).unwrap());
    }
    project_globals(memory, requests);
}

fn read_request_input(
    slot: usize,
    file_base: u32,
    destination: u32,
    offset: u32,
    size: u32,
    timestamp: u32,
    clock: u32,
    file_handle: u32,
    request_word: u32,
    data: Vec<u8>,
) -> AssetRequestInput {
    AssetRequestInput {
        slot,
        file: 7,
        file_base,
        destination: DestinationLease {
            address: destination,
            owner: 41,
            generation: 9,
            epoch: AssetEpoch(3),
        },
        offset,
        size,
        timestamp,
        clock,
        close_file_on_retire: false,
        file_handle,
        request_word,
        volume: Volume::Disc,
        bytes: data,
    }
}

fn random_word(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

#[test]
fn enqueue_matches_lifted_slot_writes_for_3000_fabricated_inputs() {
    let mut random = 0x1663_2937;
    for case in 0..3000 {
        let slot = (random_word(&mut random) as usize) % REQUEST_SLOT_COUNT;
        let file_base = random_word(&mut random);
        let destination = random_word(&mut random);
        let offset = random_word(&mut random);
        let size = random_word(&mut random);
        let timestamp = random_word(&mut random);
        let clock = random_word(&mut random);
        let handle = random_word(&mut random);
        let request_word = random_word(&mut random);
        let mut memory = Memory::default();
        let mut guest = Guest;
        let mut regs = registers();
        let record = slot_address(slot);
        memory.put32(STACK + 4, slot as u32);
        memory.put32(STACK + 8, destination);
        memory.put32(STACK + 12, offset);
        memory.put32(STACK + 16, size);
        memory.put32(STACK + 20, timestamp);
        memory.put32(record + 0x14, handle);
        memory.put32(record + 0x18, file_base);
        memory.put8(record + 0x20, 0);
        memory.put32(record + 0x24, request_word);
        memory.put32(record + 0x0c, 0xaaaa_0000 | case);
        memory.put32(record + 0x10, 0x5555_0000 | case);
        memory.put32(record + 0x40, 0);
        memory.put32(0x0074_97a8, clock);
        memory.put32(0x0066_48d0, SENTINEL);
        memory.put32(SENTINEL + 0x3c, SENTINEL);
        assert!(assets::dispatch(
            0x29370,
            &[0x29370],
            &mut memory,
            &mut guest,
            &mut regs
        ));
        assert_eq!(regs.eax, 1, "case {case}");

        let mut requests = AssetRequests::new();
        let input = read_request_input(
            slot,
            file_base,
            destination,
            offset,
            size,
            timestamp,
            clock,
            handle,
            request_word,
            vec![case as u8; 8],
        );
        let id = requests.enqueue(&Host::default(), input).unwrap();
        assert_eq!(id.slot as usize, slot);
        let snapshot = requests.legacy_slot(slot).unwrap();
        assert_eq!(
            memory.get32(record + 0x08),
            snapshot.file_offset,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x0c),
            snapshot.reserved_0c,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x10),
            snapshot.reserved_10,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x14),
            snapshot.file_handle,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x18),
            snapshot.file_base,
            "case {case}"
        );
        assert_eq!(memory.read8(record + 0x20), u8::from(snapshot.close_file));
        assert_eq!(
            memory.get32(record + 0x24),
            snapshot.request_word,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x2c),
            snapshot.aligned_size,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x30),
            snapshot.timestamp_delta,
            "case {case}"
        );
        assert_eq!(memory.get32(record + 0x34), snapshot.state, "case {case}");
        assert_eq!(
            memory.get32(record + 0x38),
            snapshot.destination,
            "case {case}"
        );
        assert_eq!(
            memory.get32(record + 0x3c),
            snapshot.previous,
            "case {case}"
        );
        assert_eq!(memory.get32(record + 0x40), snapshot.next, "case {case}");
        assert_eq!(
            memory.get32(0x0066_48d0),
            requests.legacy_head(),
            "case {case}"
        );
        assert_eq!(
            memory.get32(SENTINEL + 0x3c),
            requests.legacy_tail(),
            "case {case}"
        );
    }
}

#[test]
fn retirement_matches_lifted_unlink_and_slot_clear_for_3000_fabricated_inputs() {
    let mut random = 0x1663_2940;
    for case in 0..3000 {
        let mut requests = AssetRequests::new();
        let mut host = Host::default();
        let slots = [2usize, 5, 7];
        let mut ids = Vec::new();
        for slot in slots {
            let input = read_request_input(
                slot,
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                random_word(&mut random),
                vec![slot as u8; 4],
            );
            ids.push(requests.enqueue(&host, input).unwrap());
        }
        let released = (random_word(&mut random) as usize) % ids.len();
        let selected = if random_word(&mut random) & 1 == 0 {
            Some(ids[released])
        } else {
            Some(ids[(released + 1) % ids.len()])
        };
        let count = random_word(&mut random);
        requests.set_legacy_counters(count, selected);

        let mut lifted_memory = Memory::default();
        project_request_slots(&mut lifted_memory, &requests, &slots);
        let raw_id = u32::from(ids[released].slot) | (random_word(&mut random) & 0xffff_0000);
        let mut regs = registers();
        lifted_memory.put32(STACK + 4, raw_id);
        let mut guest = Guest;
        assert!(assets::dispatch(
            0x29400,
            &[0x29400],
            &mut lifted_memory,
            &mut guest,
            &mut regs,
        ));
        assert_eq!(regs.eax, 1, "case {case}");

        assert!(requests.retire(&mut host, ids[released]));
        assert!(host.closed.is_empty());
        let mut native_memory = Memory::default();
        project_request_slots(&mut native_memory, &requests, &slots);
        for slot in slots {
            let at = slot_address(slot);
            for offset in [
                0x08, 0x0c, 0x10, 0x14, 0x18, 0x24, 0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40,
            ] {
                assert_eq!(
                    lifted_memory.get32(at + offset),
                    native_memory.get32(at + offset),
                    "case {case}, slot {slot}, word +{offset:#x}"
                );
            }
            assert_eq!(
                lifted_memory.read8(at + 0x20),
                native_memory.read8(at + 0x20),
                "case {case}, slot {slot}, close flag"
            );
        }
        for address in [0x0066_48d0, SENTINEL + 0x3c, 0x0066_48d4, 0x0066_48d8] {
            assert_eq!(
                lifted_memory.get32(address),
                native_memory.get32(address),
                "case {case}, global {address:#x}"
            );
        }
        assert_eq!(requests.legacy_active_count(), count.wrapping_sub(1));
    }
}

#[test]
fn retired_completion_is_suppressed_and_returns_its_owned_buffer_for_reuse() {
    let mut requests = AssetRequests::new();
    let mut host = Host::default();
    let mut bytes = requests.take_owned_buffer(16);
    bytes.extend_from_slice(b"retired request");
    let original_allocation = bytes.as_ptr();
    let first = requests
        .enqueue(
            &host,
            read_request_input(4, 0, 0x1000, 0, 16, 0, 0, 0, 0, bytes),
        )
        .unwrap();
    assert!(requests.retire(&mut host, first));
    assert!(requests.complete_next(&mut host));
    assert!(
        host.writes.is_empty(),
        "retired destination was still written"
    );
    assert_eq!(
        requests.status(first),
        tsfp_engine_fileio::asset_requests::AssetRequestStatus::Retired
    );

    let mut reused = requests.take_owned_buffer(16);
    assert_eq!(reused.as_ptr(), original_allocation);
    reused.extend_from_slice(b"new request data");
    let second = requests
        .enqueue(
            &host,
            read_request_input(4, 0, 0x2000, 0, 16, 0, 0, 0, 0, reused),
        )
        .unwrap();
    assert_ne!(first.generation, second.generation);
    assert!(requests.complete_next(&mut host));
    assert_eq!(host.writes.len(), 1);
    assert_eq!(host.writes[0].0.address, 0x2000);
    assert_eq!(host.writes[0].1, b"new request data");
    assert_eq!(
        requests.status(second),
        tsfp_engine_fileio::asset_requests::AssetRequestStatus::Completed
    );
}
