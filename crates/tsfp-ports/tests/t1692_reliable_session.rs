//! T1692 Tier 1: Rust-owned reliable queue state versus lifted 0x353160/0x3532F0.
//! The 3000 seeded cases per operation are synthetic, bounded guest-memory fixtures. They do not
//! qualify retry callback/peer lifetime or live loss behavior.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use tsfp_network::syslink_reliable::{PeerId, RELIABLE_CAPACITY, ReliableQueue};
use tsfp_ports::abi::{ExtRegisters, Fpu, Xmm};
use tsfp_ports::c2r::network_session as lifted;
use tsfp_ports::memory::GuestMemory;

const QUEUE: u32 = 0x10000;
const PEER_RECORD: u32 = 0x2000;
const PEER_ID: u32 = 0x2ff0;
const PAYLOAD: u32 = 0x3000;
const STACK: u32 = 0x9000;
const ACK_MESSAGE: u32 = 0x4000;
const RELIABLE_STRIDE: u32 = 0x114;

fn registers(eax: u32, ebx: u32, esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax,
        ecx: 0,
        edx: 0,
        esp,
        ebx,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

#[derive(Clone, Default)]
struct Memory(BTreeMap<u32, u8>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

impl Memory {
    fn put16(&mut self, address: u32, value: u16) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }

    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }

    fn get16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.read8(address), self.read8(address + 1)])
    }

    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes([
            self.read8(address),
            self.read8(address + 1),
            self.read8(address + 2),
            self.read8(address + 3),
        ])
    }
}

fn payload_for(seed: u32) -> [u8; 255] {
    std::array::from_fn(|index| {
        seed.wrapping_mul(37)
            .wrapping_add((index as u32).wrapping_mul(19)) as u8
    })
}

fn fill_source(memory: &mut Memory, bytes: &[u8; 255]) {
    for (offset, byte) in bytes.iter().copied().enumerate() {
        memory.write8(PAYLOAD + offset as u32, byte);
    }
}

fn queue_fingerprint(queue: &ReliableQueue) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    queue.cursor().hash(&mut hasher);
    for slot in 0..RELIABLE_CAPACITY {
        let entry = queue.entry(slot).unwrap();
        entry.active.hash(&mut hasher);
        entry.sequence.hash(&mut hasher);
        entry.peer.hash(&mut hasher);
        entry.transmitted_len.hash(&mut hasher);
        entry.elapsed.hash(&mut hasher);
        entry.retry_count.hash(&mut hasher);
        entry.payload.hash(&mut hasher);
    }
    hasher.finish()
}

#[test]
fn enqueue_matches_lifted_353160_for_3000_seeded_records() {
    for seed in 0..3000u32 {
        let cursor = seed as usize % RELIABLE_CAPACITY;
        let peer = seed.rotate_left(11) ^ 0x7a23_19d1;
        let sequence = seed.wrapping_mul(257) as u16;
        let length = (seed.wrapping_mul(73) % 256) as u16;
        let payload = payload_for(seed);

        let mut memory = Memory::default();
        memory.put32(0x76adf0, QUEUE);
        memory.put32(0x76b120, cursor as u32);
        memory.put32(PEER_RECORD, peer);
        fill_source(&mut memory, &payload);
        memory.put32(STACK + 4, PEER_RECORD);
        memory.put16(STACK + 8, sequence);
        memory.put32(STACK + 12, PAYLOAD);
        memory.put16(STACK + 16, length);

        let mut regs = registers(0, 0, STACK);
        lifted::game_net_packet_queue_114_enqueue_at_cursor(&mut memory, &mut regs);

        let mut native = ReliableQueue::with_cursor(cursor);
        let handle = native
            .enqueue(PeerId(peer), sequence, &payload, length)
            .unwrap()
            .expect("the selected slot is free");
        assert_eq!(handle.slot(), cursor, "seed {seed}");
        assert_eq!(
            native.cursor(),
            memory.get32(0x76b120) as usize,
            "seed {seed}"
        );

        let record = QUEUE + cursor as u32 * RELIABLE_STRIDE;
        let state = native.entry(cursor).unwrap();
        assert_eq!(
            state.active,
            memory.read8(record + 0x110) & 1 != 0,
            "seed {seed}"
        );
        assert_eq!(state.peer.0, memory.get32(record), "seed {seed}");
        assert_eq!(state.sequence, memory.get16(record + 4), "seed {seed}");
        assert_eq!(
            state.transmitted_len,
            memory.get16(record + 0x106),
            "seed {seed}"
        );
        assert_eq!(state.elapsed, memory.get32(record + 0x108), "seed {seed}");
        assert_eq!(
            state.retry_count,
            memory.read8(record + 0x10c),
            "seed {seed}"
        );
        for offset in 0..255u32 {
            assert_eq!(
                state.payload[offset as usize],
                memory.read8(record + 6 + offset),
                "seed {seed}"
            );
        }
        assert_eq!(native.get(handle), Some(state), "seed {seed}");
    }
}

#[test]
fn acknowledgement_matches_lifted_3532f0_for_3000_seeded_records() {
    for seed in 0..3000u32 {
        let slot = seed as usize % RELIABLE_CAPACITY;
        let peer = seed.rotate_right(7) ^ 0x93c5_a781;
        let sequence = seed.wrapping_mul(101) as u16;
        let length = (seed.wrapping_mul(29) % 256) as u16;
        let payload = payload_for(seed ^ 0xa5a5_5a5a);

        let mut memory = Memory::default();
        memory.put32(0x76adf0, QUEUE);
        memory.put32(PEER_ID, peer);
        memory.write8(ACK_MESSAGE, 1);
        memory.put16(ACK_MESSAGE + 1, sequence);
        let record = QUEUE + slot as u32 * RELIABLE_STRIDE;
        memory.put32(record, peer);
        memory.put16(record + 4, sequence);
        memory.put16(record + 0x106, length);
        memory.put32(record + 0x108, seed.rotate_left(3));
        memory.write8(record + 0x10c, seed as u8);
        memory.write8(record + 0x110, 1);
        for (offset, byte) in payload.iter().copied().enumerate() {
            memory.write8(record + 6 + offset as u32, byte);
        }

        let mut regs = registers(ACK_MESSAGE, PEER_ID, 0);
        lifted::game_net_packet_queue_114_remove_entries_listed_in_message(&mut memory, &mut regs);

        let mut native = ReliableQueue::with_cursor(slot);
        let handle = native
            .enqueue(PeerId(peer), sequence, &payload, length)
            .unwrap()
            .expect("the selected slot is free");
        let removed = native.acknowledge(PeerId(peer), &[sequence]).unwrap();
        assert_eq!(removed, 1, "seed {seed}");
        assert!(
            native.get(handle).is_none(),
            "ack invalidates handle, seed {seed}"
        );

        let state = native.entry(slot).unwrap();
        assert!(!state.active, "seed {seed}");
        assert_eq!(state.peer.0, memory.get32(record), "seed {seed}");
        assert_eq!(state.sequence, memory.get16(record + 4), "seed {seed}");
        assert_eq!(
            state.transmitted_len,
            memory.get16(record + 0x106),
            "retained length, seed {seed}"
        );
        assert_eq!(state.elapsed, memory.get32(record + 0x108), "seed {seed}");
        assert_eq!(
            state.retry_count,
            memory.read8(record + 0x10c),
            "seed {seed}"
        );
        for offset in 0..255u32 {
            assert_eq!(
                state.payload[offset as usize],
                memory.read8(record + 6 + offset),
                "seed {seed}"
            );
        }
    }
}

#[test]
fn native_queue_rejects_unrepresentable_lengths_without_mutation() {
    let mut queue = ReliableQueue::default();
    let before = queue_fingerprint(&queue);
    let cursor = queue.cursor();
    assert!(queue.enqueue(PeerId(7), 2, &[3; 255], 256).is_err());
    assert_eq!(queue.cursor(), cursor);
    assert_eq!(queue_fingerprint(&queue), before);
}
