// SPDX-License-Identifier: GPL-3.0-or-later
//! T2394 Tier 1: fabricated differential fixtures against the retained lifted
//! 0x16C070/0x16C0D0 bodies. This checks queue contents, callback order, release,
//! and one fabricated callback-time append; it is not live-contact or Tier 2 evidence.

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use tsfp_engine_physics::{
    boundary::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_events::{
        ContactArena, ContactIndex, DeferredEvents, OwnerCallbacks, OwnerHandle, PhysicsContact,
        PhysicsEvent,
    },
    remainder,
};

const OWNER_ARRAY: u32 = 0x0010_0000;
const EVENT_ARRAY: u32 = 0x0011_0000;
const SOURCE: u32 = 0x0012_0000;
const STACK: u32 = 0x0013_0000;
const OWNER_BASE: u32 = 0x0020_0000;
const CONTACT_WORD: u32 = 0x0030_0000;
const CALLBACK: u32 = 0x00ca_f000;
const FREE: u32 = 0x0003_e1e0;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<std::collections::HashMap<u32, u8>>>);

impl Memory {
    fn write32(&self, address: u32, value: u32) {
        let mut bytes = self.0.borrow_mut();
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            bytes.insert(address.wrapping_add(offset as u32), byte);
        }
    }

    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|offset| {
            self.0
                .borrow()
                .get(&address.wrapping_add(offset as u32))
                .copied()
                .unwrap_or(0)
        }))
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

fn registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
        ebx: 0,
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum LiftedCall {
    Contact {
        callback: u32,
        owner: u32,
        row: [u32; 4],
    },
    Free(u32),
}

#[derive(Default)]
struct LiftedServices {
    memory: Memory,
    calls: Vec<LiftedCall>,
    append_once: Option<(u32, u32, [u32; 2])>,
}

impl GuestCall for LiftedServices {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        if address == FREE {
            self.calls
                .push(LiftedCall::Free(self.memory.read32(regs.esp)));
            return;
        }
        let owner = self.memory.read32(regs.esp);
        let row_pointer = self.memory.read32(regs.esp.wrapping_add(4));
        let row = std::array::from_fn(|word| {
            self.memory
                .read32(row_pointer.wrapping_add(word as u32 * 4))
        });
        self.calls.push(LiftedCall::Contact {
            callback: address,
            owner,
            row,
        });
        if address == CALLBACK {
            if let Some((new_owner, other, normals)) = self.append_once.take() {
                let count = self.memory.read32(0x74c490);
                let owners = self.memory.read32(0x74c484);
                let events = self.memory.read32(0x74c474);
                self.memory
                    .write32(owners.wrapping_add(count.wrapping_mul(4)), new_owner);
                let row_base = events.wrapping_add(count.wrapping_mul(0x10));
                self.memory.write32(row_base, other);
                self.memory.write32(row_base + 4, normals[0]);
                self.memory.write32(row_base + 8, normals[1]);
                self.memory.write32(row_base + 12, CONTACT_WORD);
                self.memory.write32(0x74c490, count.wrapping_add(1));
            }
        }
    }
}

#[derive(Default)]
struct NativeServices {
    callbacks: HashSet<u32>,
    calls: Vec<(u32, u32, [u32; 2])>,
    append_once: Option<(u32, u32, [u32; 2])>,
    contact: Option<tsfp_engine_physics::rea_events::ContactLease>,
}

impl OwnerCallbacks for NativeServices {
    fn has_callback(&self, owner: OwnerHandle) -> bool {
        self.callbacks.contains(&owner.0)
    }

    fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
        let event = queue.row(slot).unwrap();
        self.calls.push((
            owner.0,
            event.other.map_or(0, |other| other.0),
            event.horizontal_normal_bits,
        ));
        if let (Some((new_owner, other, normals)), Some(contact)) =
            (self.append_once.take(), self.contact.clone())
        {
            let appended = PhysicsEvent {
                recipient: OwnerHandle(new_owner),
                other: Some(OwnerHandle(other)),
                horizontal_normal_bits: normals,
                contact,
            };
            assert!(queue.enqueue(appended));
        }
    }
}

fn make_arena() -> ContactArena {
    ContactArena::new(vec![PhysicsContact {
        kind: 0,
        owner_a: Some(OwnerHandle(0x101)),
        owner_b: Some(OwnerHandle(0x202)),
        horizontal_normal: Some([f32::from_bits(0x3f00_0001), f32::from_bits(0x8000_0000)]),
        relative_speed_bits: Some(0x3e80_1234),
        next: None,
        guest_next: None,
        guest_identity: None,
    }])
}

fn event(arena: &ContactArena, recipient: u32, other: u32, normals: [u32; 2]) -> PhysicsEvent {
    PhysicsEvent {
        recipient: OwnerHandle(recipient),
        other: (other != 0).then_some(OwnerHandle(other)),
        horizontal_normal_bits: normals,
        contact: arena.lease(ContactIndex(0)).unwrap(),
    }
}

fn next(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

#[test]
fn append_matches_lifted_for_2500_fabricated_rows() {
    let arena = make_arena();
    let mut seed = 0x2394_16c0;
    for case in 0..2_500 {
        let capacity = (next(&mut seed) % 9 + 1) as usize;
        let initial_count = (next(&mut seed) as usize) % (capacity + 1);
        let new_owner = next(&mut seed);
        let new_words = [
            next(&mut seed),
            next(&mut seed),
            next(&mut seed),
            CONTACT_WORD,
        ];
        let mut memory = Memory::default();
        memory.write32(0x74c484, OWNER_ARRAY);
        memory.write32(0x74c474, EVENT_ARRAY);
        memory.write32(0x74c47c, capacity as u32);
        memory.write32(0x74c490, initial_count as u32);
        let mut native = DeferredEvents::new(capacity);
        for slot in 0..initial_count {
            let owner = next(&mut seed);
            let row = [
                next(&mut seed),
                next(&mut seed),
                next(&mut seed),
                CONTACT_WORD,
            ];
            memory.write32(OWNER_ARRAY + slot as u32 * 4, owner);
            for (word, value) in row.into_iter().enumerate() {
                memory.write32(EVENT_ARRAY + slot as u32 * 0x10 + word as u32 * 4, value);
            }
            assert!(native.enqueue(event(&arena, owner, row[0], [row[1], row[2]])));
        }
        for (word, value) in new_words.into_iter().enumerate() {
            memory.write32(SOURCE + word as u32 * 4, value);
        }
        memory.write32(STACK + 4, new_owner);
        let mut regs = registers(STACK);
        regs.ecx = SOURCE;
        remainder::f16c070(&mut memory, &mut LiftedServices::default(), &mut regs);

        let mut native_input = event(
            &arena,
            new_owner,
            new_words[0],
            [new_words[1], new_words[2]],
        );
        native_input.horizontal_normal_bits = [new_words[1], new_words[2]];
        let accepted = initial_count < capacity;
        assert_eq!(native.enqueue(native_input), accepted, "case {case}");
        assert_eq!(
            memory.read32(0x74c490) as usize,
            native.count(),
            "count case {case}"
        );
        for slot in 0..native.count() {
            let row = native.row(slot).unwrap();
            assert_eq!(
                memory.read32(OWNER_ARRAY + slot as u32 * 4),
                row.recipient.0,
                "owner case {case}"
            );
            let base = EVENT_ARRAY + slot as u32 * 0x10;
            assert_eq!(
                memory.read32(base),
                row.other.map_or(0, |owner| owner.0),
                "other case {case}"
            );
            assert_eq!(
                memory.read32(base + 4),
                row.horizontal_normal_bits[0],
                "normal-x case {case}"
            );
            assert_eq!(
                memory.read32(base + 8),
                row.horizontal_normal_bits[1],
                "normal-z case {case}"
            );
            assert_eq!(memory.read32(base + 12), CONTACT_WORD, "lease case {case}");
            assert_eq!(row.contact.index(), ContactIndex(0));
        }
    }
}

#[test]
fn drain_matches_lifted_for_2500_fabricated_queues() {
    let arena = make_arena();
    let mut seed = 0x2394_0d00;
    for case in 0..2_500 {
        let capacity = (next(&mut seed) % 9 + 1) as usize;
        let count = (next(&mut seed) as usize) % (capacity + 1);
        let mut memory = Memory::default();
        memory.write32(0x74c484, OWNER_ARRAY);
        memory.write32(0x74c474, EVENT_ARRAY);
        memory.write32(0x74c47c, capacity as u32);
        memory.write32(0x74c490, count as u32);
        let mut native = DeferredEvents::new(capacity);
        let mut native_services = NativeServices::default();
        for slot in 0..count {
            let owner = if next(&mut seed) % 7 == 0 {
                0
            } else {
                OWNER_BASE + ((next(&mut seed) % 8) * 0x200)
            };
            let other = if next(&mut seed) % 5 == 0 {
                0
            } else {
                next(&mut seed)
            };
            let normals = [next(&mut seed), next(&mut seed)];
            let callback = next(&mut seed) % 4 != 0;
            let row_base = EVENT_ARRAY + slot as u32 * 0x10;
            memory.write32(OWNER_ARRAY + slot as u32 * 4, owner);
            memory.write32(row_base, other);
            memory.write32(row_base + 4, normals[0]);
            memory.write32(row_base + 8, normals[1]);
            memory.write32(row_base + 12, CONTACT_WORD);
            if callback && owner != 0 {
                memory.write32(owner + 0x170, CALLBACK);
                native_services.callbacks.insert(owner);
            }
            assert!(native.enqueue(event(&arena, owner, other, normals)));
        }
        let mut lifted = LiftedServices {
            memory: memory.clone(),
            calls: Vec::new(),
            append_once: None,
        };
        remainder::f16c0d0(&mut memory, &mut lifted, &mut registers(STACK));
        let report = native.drain(&mut native_services);

        let lifted_callbacks: Vec<_> = lifted
            .calls
            .iter()
            .filter_map(|call| match call {
                LiftedCall::Contact { owner, row, .. } => Some((*owner, row[0], [row[1], row[2]])),
                LiftedCall::Free(_) => None,
            })
            .collect();
        assert_eq!(
            native_services.calls, lifted_callbacks,
            "callback trace case {case}"
        );
        let freed: Vec<_> = lifted
            .calls
            .iter()
            .filter_map(|call| match call {
                LiftedCall::Free(pointer) => Some(*pointer),
                LiftedCall::Contact { .. } => None,
            })
            .collect();
        assert_eq!(freed, [OWNER_ARRAY, EVENT_ARRAY], "free order case {case}");
        assert_eq!(
            memory.read32(0x74c484),
            0,
            "owner array release case {case}"
        );
        assert_eq!(
            memory.read32(0x74c474),
            0,
            "event array release case {case}"
        );
        assert_eq!(memory.read32(0x74c47c), 0, "capacity release case {case}");
        assert_eq!(memory.read32(0x74c490), 0, "count release case {case}");
        assert!(report.released, "typed release case {case}");
    }
}

#[test]
fn callback_enqueue_is_drained_in_the_same_pass_like_lifted_for_fabricated_rows() {
    let arena = Box::leak(Box::new(make_arena()));
    let contact = arena.lease(ContactIndex(0)).unwrap();
    let (first_owner, appended_owner, other) = (OWNER_BASE + 0x200, OWNER_BASE + 0x400, 0x303);
    let initial = [
        (first_owner, 0x111, [0x3f80_0000, 0x4000_0000]),
        (OWNER_BASE + 0x600, 0x222, [0x4040_0000, 0x4080_0000]),
    ];
    let appended = (appended_owner, other, [0x40a0_0000, 0x40c0_0000]);
    let mut memory = Memory::default();
    memory.write32(0x74c484, OWNER_ARRAY);
    memory.write32(0x74c474, EVENT_ARRAY);
    memory.write32(0x74c47c, 3);
    memory.write32(0x74c490, initial.len() as u32);
    for (slot, (owner, other, normals)) in initial.into_iter().enumerate() {
        memory.write32(OWNER_ARRAY + slot as u32 * 4, owner);
        let base = EVENT_ARRAY + slot as u32 * 0x10;
        memory.write32(base, other);
        memory.write32(base + 4, normals[0]);
        memory.write32(base + 8, normals[1]);
        memory.write32(base + 12, CONTACT_WORD);
        memory.write32(owner + 0x170, CALLBACK);
    }
    memory.write32(appended_owner + 0x170, CALLBACK);

    let mut lifted = LiftedServices {
        memory: memory.clone(),
        calls: Vec::new(),
        append_once: Some(appended),
    };
    remainder::f16c0d0(&mut memory, &mut lifted, &mut registers(STACK));

    let mut native = DeferredEvents::new(3);
    for (owner, other, normals) in initial {
        assert!(native.enqueue(PhysicsEvent {
            recipient: OwnerHandle(owner),
            other: Some(OwnerHandle(other)),
            horizontal_normal_bits: normals,
            contact: contact.clone(),
        }));
    }
    let mut handlers = NativeServices {
        callbacks: [first_owner, OWNER_BASE + 0x600, appended_owner]
            .into_iter()
            .collect(),
        calls: Vec::new(),
        append_once: Some(appended),
        contact: Some(contact),
    };
    native.drain(&mut handlers);

    let lifted_callbacks: Vec<_> = lifted
        .calls
        .iter()
        .filter_map(|call| match call {
            LiftedCall::Contact { owner, row, .. } => Some((*owner, row[0], [row[1], row[2]])),
            LiftedCall::Free(_) => None,
        })
        .collect();
    assert_eq!(handlers.calls, lifted_callbacks);
    assert_eq!(
        handlers.calls,
        [
            (OWNER_BASE + 0x600, 0x222, [0x4040_0000, 0x4080_0000]),
            (appended_owner, other, appended.2),
            (first_owner, 0x111, [0x3f80_0000, 0x4000_0000]),
        ]
    );
}
