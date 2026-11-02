// SPDX-License-Identifier: GPL-3.0-or-later
//! T2394 Tier 1 fabricated comparisons for the bounded typed motion operations.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use tsfp_engine_physics::rea_events::{
    ContactArena, ContactIndex, DeferredEvents, OwnerCallbacks, OwnerHandle, PhysicsContact,
    PhysicsEvent,
};
use tsfp_gameplay::motion::{ActorId, CandidatePath, MotionFrame, PlayerId};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::gameplay_player,
    memory::GuestMemory,
};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_call(va: u32, regs: *mut u32) -> i32;
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
    fn t20_lifted_lookup(va: u32) -> Option<unsafe extern "C" fn()>;
}

const ARENA: usize = (1 << 32) + 0x1_0000;

struct Pair {
    c: *mut u8,
    rust: *mut u8,
}
unsafe impl Send for Pair {}

impl Pair {
    fn new() -> Self {
        let map = || unsafe { mmap(std::ptr::null_mut(), ARENA, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
        let pair = Self {
            c: map(),
            rust: map(),
        };
        assert_ne!(pair.c as isize, -1, "C guest arena mmap failed");
        assert_ne!(pair.rust as isize, -1, "Rust guest arena mmap failed");
        unsafe { shim_set_base(pair.c) };
        pair
    }

    fn put32(&mut self, address: u32, value: u32) {
        let bytes = value.to_le_bytes();
        for base in [self.c, self.rust] {
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(address as usize), 4);
            }
        }
    }

    fn get(&self, rust: bool, address: u32) -> u32 {
        let base = if rust { self.rust } else { self.c };
        u32::from_le(unsafe { std::ptr::read_unaligned(base.add(address as usize).cast()) })
    }

    fn clear(&mut self, base: u32, len: usize) {
        unsafe {
            std::ptr::write_bytes(self.c.add(base as usize), 0, len);
            std::ptr::write_bytes(self.rust.add(base as usize), 0, len);
        }
    }
}

fn pair() -> &'static Mutex<Pair> {
    static PAIR: OnceLock<Mutex<Pair>> = OnceLock::new();
    PAIR.get_or_init(|| Mutex::new(Pair::new()))
}

fn locked_pair() -> std::sync::MutexGuard<'static, Pair> {
    pair()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn typed_deferred_append_matches_original_lifted_queue_for_2500_fabricated_rows() {
    const OWNER_ARRAY: u32 = 0x0010_0000;
    const EVENT_ARRAY: u32 = 0x0011_0000;
    const SOURCE: u32 = 0x0012_0000;
    const STACK: u32 = 0x0013_0000;

    assert_eq!(std::env::var("TSFP_REF_LIFTED").as_deref(), Ok("1"));
    assert!(unsafe { t20_lifted_lookup(0x0016_c070) }.is_some());

    let mut pair = locked_pair();
    let mut rng = Rng(0x3396_16c0);
    for case in 0..2_500 {
        for (base, size) in [
            (0x0074_c470, 0x40),
            (OWNER_ARRAY, 0x20),
            (EVENT_ARRAY, 0x20),
            (SOURCE, 0x20),
            (STACK, 0x20),
        ] {
            pair.clear(base, size);
        }

        let recipient = OwnerHandle(0x0020_0000 + case * 16);
        let other_raw = if case % 7 == 0 {
            0
        } else {
            0x0021_0000 + rng.next() % 0x1000
        };
        let normal_bits = [rng.next(), rng.next()];
        let contact_address = 0x0030_0000 + case * 0x40;
        for (index, value) in [other_raw, normal_bits[0], normal_bits[1], contact_address]
            .into_iter()
            .enumerate()
        {
            pair.put32(SOURCE + index as u32 * 4, value);
        }
        pair.put32(0x0074_c484, OWNER_ARRAY);
        pair.put32(0x0074_c474, EVENT_ARRAY);
        pair.put32(0x0074_c47c, 1);
        pair.put32(0x0074_c490, 0);
        pair.put32(OWNER_ARRAY, 0);
        pair.put32(STACK + 4, recipient.0);

        let mut regs = [0, SOURCE, 0, 0, STACK];
        assert_eq!(
            unsafe { shim_call(0x0016_c070, regs.as_mut_ptr()) },
            1,
            "case {case}"
        );

        let contact = PhysicsContact {
            kind: (rng.next() % 4) as u16,
            owner_a: Some(OwnerHandle(0x0040_0000 + case * 8)),
            owner_b: Some(OwnerHandle(0x0040_0004 + case * 8)),
            horizontal_normal: Some(normal_bits.map(f32::from_bits)),
            relative_speed_bits: Some(rng.next()),
            next: None,
            guest_next: None,
            guest_identity: None,
        };
        let arena = ContactArena::new(vec![contact.clone()]);
        let mut typed = DeferredEvents::new(1);
        assert!(typed.enqueue(PhysicsEvent {
            recipient,
            other: (other_raw != 0).then_some(OwnerHandle(other_raw)),
            horizontal_normal_bits: normal_bits,
            contact: arena.lease(ContactIndex(0)).unwrap(),
        }));

        assert_eq!(pair.get(false, 0x0074_c490), 1, "case {case}: lifted count");
        assert_eq!(
            pair.get(false, OWNER_ARRAY),
            recipient.0,
            "case {case}: lifted recipient"
        );
        assert_eq!(
            pair.get(false, EVENT_ARRAY),
            other_raw,
            "case {case}: lifted other"
        );
        assert_eq!(
            pair.get(false, EVENT_ARRAY + 4),
            normal_bits[0],
            "case {case}: lifted normal x"
        );
        assert_eq!(
            pair.get(false, EVENT_ARRAY + 8),
            normal_bits[1],
            "case {case}: lifted normal y"
        );

        let row = typed.row(0).expect("typed row");
        assert_eq!(typed.count(), 1);
        assert_eq!(row.recipient, recipient);
        assert_eq!(
            row.other,
            (other_raw != 0).then_some(OwnerHandle(other_raw))
        );
        assert_eq!(row.horizontal_normal_bits, normal_bits);
        let leased_contact = row
            .contact
            .with(|leased| {
                (
                    leased.kind,
                    leased.owner_a,
                    leased.owner_b,
                    leased
                        .horizontal_normal
                        .map(|normal| normal.map(f32::to_bits)),
                    leased.relative_speed_bits,
                    leased.next,
                )
            })
            .expect("leased contact");
        assert_eq!(
            leased_contact,
            (
                contact.kind,
                contact.owner_a,
                contact.owner_b,
                contact
                    .horizontal_normal
                    .map(|normal| normal.map(f32::to_bits)),
                contact.relative_speed_bits,
                contact.next,
            ),
            "case {case}: leased contact bits"
        );
    }
}

#[derive(Default)]
struct DrainRecorder {
    callbacks: HashSet<u32>,
    delivered: Vec<(u32, usize, u32, [u32; 2])>,
}

impl OwnerCallbacks for DrainRecorder {
    fn has_callback(&self, owner: OwnerHandle) -> bool {
        self.callbacks.contains(&owner.0)
    }

    fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
        let event = queue.row(slot).expect("queued row exists during callback");
        self.delivered.push((
            owner.0,
            slot,
            event.other.map_or(0, |other| other.0),
            event.horizontal_normal_bits,
        ));
    }
}

#[test]
fn typed_deferred_drain_matches_original_lifted_queue_for_2500_fabricated_rows() {
    const OWNER_ARRAY: u32 = 0x0010_0000;
    const EVENT_ARRAY: u32 = 0x0011_0000;
    const STACK: u32 = 0x0013_0000;
    const OWNER_BASE: u32 = 0x0020_0000;
    const CALLBACK: u32 = 0x00ca_f000;
    const FREE: u32 = 0x0003_e1e0;

    assert_eq!(std::env::var("TSFP_REF_LIFTED").as_deref(), Ok("1"));
    assert!(unsafe { t20_lifted_lookup(0x0016_c0d0) }.is_some());

    let mut pair = locked_pair();
    let mut rng = Rng(0x3396_d0d0);
    for case in 0..2_500 {
        // The C shim's call trace is global; isolate this randomized case.
        unsafe { shim_script(std::ptr::null(), 0) };
        for (base, size) in [
            (0x0074_c470, 0x40),
            (OWNER_ARRAY, 0x80),
            (EVENT_ARRAY, 0x100),
            (OWNER_BASE, 0x1000),
            (STACK, 0x20),
        ] {
            pair.clear(base, size);
        }

        let capacity = (rng.next() % 8 + 1) as usize;
        let count = (rng.next() as usize) % (capacity + 1);
        pair.put32(0x0074_c484, OWNER_ARRAY);
        pair.put32(0x0074_c474, EVENT_ARRAY);
        pair.put32(0x0074_c47c, capacity as u32);
        pair.put32(0x0074_c490, count as u32);

        let mut typed = DeferredEvents::new(capacity);
        let mut recorder = DrainRecorder::default();
        for slot in 0..count {
            let owner = if slot % 7 == 0 {
                0
            } else {
                OWNER_BASE + slot as u32 * 0x100
            };
            let other = if slot % 5 == 0 {
                0
            } else {
                0x0030_0000 + rng.next() % 0x1000
            };
            let normals = [rng.next(), rng.next()];
            let contact_ptr = 0x0040_0000 + case * 0x100 + slot as u32 * 0x40;
            let callback = if slot % 3 == 0 { 0 } else { CALLBACK };

            pair.put32(OWNER_ARRAY + slot as u32 * 4, owner);
            pair.put32(EVENT_ARRAY + slot as u32 * 0x10, other);
            pair.put32(EVENT_ARRAY + slot as u32 * 0x10 + 4, normals[0]);
            pair.put32(EVENT_ARRAY + slot as u32 * 0x10 + 8, normals[1]);
            pair.put32(EVENT_ARRAY + slot as u32 * 0x10 + 12, contact_ptr);
            if owner != 0 {
                pair.put32(owner + 0x170, callback);
                if callback != 0 {
                    recorder.callbacks.insert(owner);
                }
            }

            let contact = PhysicsContact {
                kind: 0,
                owner_a: Some(OwnerHandle(owner)),
                owner_b: Some(OwnerHandle(other)),
                horizontal_normal: Some(normals.map(f32::from_bits)),
                relative_speed_bits: Some(0),
                next: None,
                guest_next: None,
                guest_identity: None,
            };
            assert!(
                typed.enqueue(PhysicsEvent {
                    recipient: OwnerHandle(owner),
                    other: (other != 0).then_some(OwnerHandle(other)),
                    horizontal_normal_bits: normals,
                    contact: ContactArena::new(vec![contact])
                        .lease(ContactIndex(0))
                        .unwrap(),
                })
            );
        }

        let expected = typed.drain(&mut recorder);
        let regs = [0, 0, 0, 0, STACK];
        let mut regs = regs;
        assert_eq!(
            unsafe { shim_call(0x0016_c0d0, regs.as_mut_ptr()) },
            1,
            "case {case}"
        );

        let trace_len = unsafe { shim_trace_len() };
        let mut c_callbacks = Vec::new();
        let mut c_frees = Vec::new();
        for index in 0..trace_len {
            let words = unsafe { std::slice::from_raw_parts(shim_trace_entry(index), 16) };
            match words[0] {
                CALLBACK => {
                    let row = words[4];
                    let slot = (row - EVENT_ARRAY) as usize / 0x10;
                    assert_eq!(
                        row,
                        EVENT_ARRAY + slot as u32 * 0x10,
                        "case {case}: row alignment"
                    );
                    c_callbacks.push((
                        words[3],
                        slot,
                        pair.get(false, row),
                        [pair.get(false, row + 4), pair.get(false, row + 8)],
                    ));
                }
                FREE => c_frees.push(words[3]),
                target => panic!("unexpected lifted callee {target:#x} in case {case}"),
            }
        }

        assert_eq!(
            c_callbacks, recorder.delivered,
            "case {case}: lifted LIFO callback rows"
        );
        assert_eq!(expected.delivered, c_callbacks.len());
        assert_eq!(expected.released, true);
        assert_eq!(
            c_frees,
            [OWNER_ARRAY, EVENT_ARRAY],
            "case {case}: release order"
        );
        assert_eq!(
            pair.get(false, 0x0074_c484),
            0,
            "case {case}: owner array cleared"
        );
        assert_eq!(
            pair.get(false, 0x0074_c474),
            0,
            "case {case}: event array cleared"
        );
        assert_eq!(
            pair.get(false, 0x0074_c47c),
            0,
            "case {case}: capacity cleared"
        );
        assert_eq!(
            pair.get(false, 0x0074_c490),
            0,
            "case {case}: count cleared"
        );
    }
}

struct ReentrantDrainRecorder {
    callbacks: HashSet<u32>,
    delivered: Vec<(u32, usize, u32, [u32; 2])>,
    append: Option<PhysicsEvent>,
}

impl OwnerCallbacks for ReentrantDrainRecorder {
    fn has_callback(&self, owner: OwnerHandle) -> bool {
        self.callbacks.contains(&owner.0)
    }

    fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
        let event = queue.row(slot).expect("queued row exists during callback");
        self.delivered.push((
            owner.0,
            slot,
            event.other.map_or(0, |other| other.0),
            event.horizontal_normal_bits,
        ));
        if let Some(appended) = self.append.take() {
            assert!(
                queue.enqueue(appended),
                "callback append reuses popped slot"
            );
        }
    }
}

fn fabricated_event(
    recipient: u32,
    other: u32,
    normal_bits: [u32; 2],
    _contact_address: u32,
) -> PhysicsEvent {
    PhysicsEvent {
        recipient: OwnerHandle(recipient),
        other: Some(OwnerHandle(other)),
        horizontal_normal_bits: normal_bits,
        contact: ContactArena::new(vec![PhysicsContact {
            kind: 0,
            owner_a: Some(OwnerHandle(recipient)),
            owner_b: Some(OwnerHandle(other)),
            horizontal_normal: Some(normal_bits.map(f32::from_bits)),
            relative_speed_bits: Some(0),
            next: None,
            guest_next: None,
            guest_identity: None,
        }])
        .lease(ContactIndex(0))
        .expect("fabricated contact lease"),
    }
}

#[test]
fn typed_deferred_drain_matches_original_lifted_reentrant_append_for_fabricated_rows() {
    const OWNER_ARRAY: u32 = 0x0010_0000;
    const EVENT_ARRAY: u32 = 0x0011_0000;
    const STACK: u32 = 0x0013_0000;
    const OWNER_BASE: u32 = 0x0020_0000;
    const CALLBACK: u32 = 0x00ca_f000;
    const FREE: u32 = 0x0003_e1e0;
    const TOP: u32 = OWNER_BASE + 0x100;
    const OLDER: u32 = OWNER_BASE;
    const APPENDED: u32 = OWNER_BASE + 0x200;
    const TOP_ROW: [u32; 4] = [0x0030_0100, 0x3f80_0000, 0xbf00_0000, 0x0040_0100];
    const OLDER_ROW: [u32; 4] = [0x0030_0000, 0x3e80_0000, 0xbe00_0000, 0x0040_0000];
    const APPENDED_ROW: [u32; 4] = [0x0030_0200, 0x4000_0000, 0xc000_0000, 0x0040_0200];

    assert_eq!(std::env::var("TSFP_REF_LIFTED").as_deref(), Ok("1"));
    assert!(unsafe { t20_lifted_lookup(0x0016_c0d0) }.is_some());

    let mut pair = locked_pair();
    unsafe { shim_script(std::ptr::null(), 0) };
    for (base, size) in [
        (0x0074_c470, 0x40),
        (OWNER_ARRAY, 0x20),
        (EVENT_ARRAY, 0x40),
        (OWNER_BASE, 0x400),
        (STACK, 0x20),
    ] {
        pair.clear(base, size);
    }
    pair.put32(0x0074_c484, OWNER_ARRAY);
    pair.put32(0x0074_c474, EVENT_ARRAY);
    pair.put32(0x0074_c47c, 2);
    pair.put32(0x0074_c490, 2);
    for owner in [OLDER, TOP, APPENDED] {
        pair.put32(owner + 0x170, CALLBACK);
    }
    pair.put32(OWNER_ARRAY, OLDER);
    pair.put32(OWNER_ARRAY + 4, TOP);
    for (index, value) in OLDER_ROW.into_iter().enumerate() {
        pair.put32(EVENT_ARRAY + index as u32 * 4, value);
    }
    for (index, value) in TOP_ROW.into_iter().enumerate() {
        pair.put32(EVENT_ARRAY + 0x10 + index as u32 * 4, value);
    }

    // The fabricated first callback enqueues one row into the just-popped slot.
    // The lifted C callback stub applies the same guest-memory writes before it returns.
    let writes = [
        0,
        OWNER_ARRAY + 4,
        APPENDED,
        0,
        EVENT_ARRAY + 0x10,
        APPENDED_ROW[0],
        0,
        EVENT_ARRAY + 0x14,
        APPENDED_ROW[1],
        0,
        EVENT_ARRAY + 0x18,
        APPENDED_ROW[2],
        0,
        EVENT_ARRAY + 0x1c,
        APPENDED_ROW[3],
        0,
        0x0074_c490,
        2,
    ];
    unsafe { shim_script_writes(writes.as_ptr(), 6) };

    let mut typed = DeferredEvents::new(2);
    assert!(typed.enqueue(fabricated_event(
        OLDER,
        OLDER_ROW[0],
        [OLDER_ROW[1], OLDER_ROW[2]],
        OLDER_ROW[3]
    )));
    assert!(typed.enqueue(fabricated_event(
        TOP,
        TOP_ROW[0],
        [TOP_ROW[1], TOP_ROW[2]],
        TOP_ROW[3]
    )));
    let mut recorder = ReentrantDrainRecorder {
        callbacks: [OLDER, TOP, APPENDED].into_iter().collect(),
        delivered: Vec::new(),
        append: Some(fabricated_event(
            APPENDED,
            APPENDED_ROW[0],
            [APPENDED_ROW[1], APPENDED_ROW[2]],
            APPENDED_ROW[3],
        )),
    };
    let expected = typed.drain(&mut recorder);

    let mut regs = [0, 0, 0, 0, STACK];
    assert_eq!(unsafe { shim_call(0x0016_c0d0, regs.as_mut_ptr()) }, 1);
    let mut c_callbacks = Vec::new();
    let mut c_frees = Vec::new();
    for index in 0..unsafe { shim_trace_len() } {
        let words = unsafe { std::slice::from_raw_parts(shim_trace_entry(index), 16) };
        match words[0] {
            CALLBACK => {
                let row = words[4];
                let slot = (row - EVENT_ARRAY) as usize / 0x10;
                c_callbacks.push((words[3], slot, words[13], [words[14], words[15]]));
            }
            FREE => c_frees.push(words[3]),
            target => panic!("unexpected lifted callee {target:#x}"),
        }
    }
    assert_eq!(recorder.delivered, c_callbacks);
    assert_eq!(
        c_callbacks.iter().map(|row| row.1).collect::<Vec<_>>(),
        [1, 1, 0]
    );
    assert_eq!(expected.delivered, 3);
    assert!(expected.released);
    assert_eq!(c_frees, [OWNER_ARRAY, EVENT_ARRAY]);
    assert_eq!(pair.get(false, 0x0074_c484), 0);
    assert_eq!(pair.get(false, 0x0074_c474), 0);
    assert_eq!(pair.get(false, 0x0074_c47c), 0);
    assert_eq!(pair.get(false, 0x0074_c490), 0);
}

struct View(*mut u8);
impl GuestMemory for View {
    fn read8(&self, address: u32) -> u8 {
        unsafe { *self.0.add(address as usize) }
    }
    fn write8(&mut self, address: u32, value: u8) {
        unsafe { *self.0.add(address as usize) = value }
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

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn finite(&mut self, span: i32) -> f32 {
        ((self.next() % (span as u32 * 2 + 1)) as i32 - span) as f32 / 1000.0
    }
}

#[test]
fn d0ca0_pose_publication_matches_lifted_replacement_for_2500_inputs() {
    const PLAYER: u32 = 0x0058_0000;
    const ACTOR: u32 = 0x0059_0000;
    let mut pair = locked_pair();
    let mut rng = Rng(0x2394_d0ca);
    for case in 0..2_500 {
        pair.clear(PLAYER, 0x1000);
        pair.clear(ACTOR, 0x1000);
        pair.put32(0x007b_0c7c, PLAYER);
        pair.put32(PLAYER + 0x14, ACTOR);
        let state = if rng.next() % 4 == 0 {
            0x400
        } else {
            rng.next()
        };
        pair.put32(PLAYER + 0x114, state);
        let position = [rng.finite(10_000), rng.finite(10_000), rng.finite(10_000)];
        for (offset, value) in [0xa8, 0xac, 0xb0].into_iter().zip(position) {
            pair.put32(PLAYER + offset, value.to_bits());
        }
        let initial = [0x1122_3344, 0x5566_7788, 0x99aa_bbcc];
        for (offset, value) in [0x64, 0x68, 0x6c].into_iter().zip(initial) {
            pair.put32(ACTOR + offset, value);
        }
        let mut regs = [0x1111, 0x2222, 0x3333, 0x4444, 0x0030_0000, 0x6666];
        assert_eq!(
            unsafe { shim_call(0x000d_0ca0, regs.as_mut_ptr()) },
            1,
            "case {case}"
        );
        let frame = MotionFrame {
            player: PlayerId(PLAYER),
            actor: ActorId(ACTOR),
            fixed_update: case as u64,
            player_state: state,
            player_position: position,
            accumulated_planar: [0.0; 2],
            additive_position: [0.0; 3],
            stored_intent: [0.0; 2],
            body_velocity: None,
        };
        let expected = frame
            .actor_pose_publication()
            .map_or(initial, |publication| {
                publication.position.map(f32::to_bits)
            });
        assert_eq!(
            [
                pair.get(false, ACTOR + 0x64),
                pair.get(false, ACTOR + 0x68),
                pair.get(false, ACTOR + 0x6c)
            ],
            expected,
            "case {case}"
        );
    }
}

#[test]
fn d2b80_accumulator_matches_lifted_body_for_2500_inputs() {
    const PLAYER: u32 = 0x0050_0000;
    const ACTOR: u32 = 0x0051_0000;
    const EXTENSION: u32 = 0x0052_0000;
    const INPUT: u32 = 0x0053_0000;
    const BASIS: u32 = 0x0054_0000;
    const SECOND_BASIS: u32 = 0x0055_0000;
    const SCALAR_OUT: u32 = 0x0056_0000;
    const STACK: u32 = 0x0057_0000;
    const INJECTION: f32 = 0.01;
    const DAMPING: f32 = 0.97;

    let mut pair = locked_pair();
    pair.put32(0x007b_0c7c, PLAYER);
    pair.put32(0x0047_5d0c, 0.017_453_292_5f32.to_bits());
    pair.put32(0x0047_5d1c, INJECTION.to_bits());
    pair.put32(0x0047_d684, DAMPING.to_bits());
    let mut rng = Rng(0x2394_d2b8);
    for case in 0..2_500 {
        for base in [
            PLAYER,
            ACTOR,
            EXTENSION,
            INPUT,
            BASIS,
            SECOND_BASIS,
            SCALAR_OUT,
            STACK,
        ] {
            pair.clear(base, if base == PLAYER { 0x1000 } else { 0x100 });
        }
        pair.put32(0x007b_0c7c, PLAYER);
        pair.put32(PLAYER + 0x14, ACTOR);
        pair.put32(PLAYER + 0x100, rng.finite(3_000).to_bits());
        pair.put32(PLAYER + 0x11c, 0);
        pair.put32(ACTOR + 0x7c, EXTENSION);
        let old = [rng.finite(8_000), rng.finite(8_000)];
        pair.put32(PLAYER + 0x120, old[0].to_bits());
        pair.put32(PLAYER + 0x124, old[1].to_bits());
        let input = rng.finite(4_000);
        pair.put32(INPUT, input.to_bits());
        pair.put32(INPUT + 4, rng.finite(4_000).to_bits());
        pair.put32(STACK, 0);
        pair.put32(STACK + 4, SECOND_BASIS);
        pair.put32(STACK + 8, SCALAR_OUT);
        pair.put32(STACK + 12, 0);
        let mut regs = registers(STACK);
        regs.eax = BASIS;
        regs.ecx = INPUT;
        regs.edx = SCALAR_OUT;
        let mut view = View(pair.rust);
        let mut stats = MotionStats;
        assert!(
            gameplay_player::run(0xd2b80, &mut view, &mut stats, &mut regs),
            "case {case}"
        );
        let yaw = [
            f32::from_bits(pair.get(true, BASIS)),
            f32::from_bits(pair.get(true, BASIS + 4)),
        ];
        let mut frame = MotionFrame {
            player: PlayerId(PLAYER),
            actor: ActorId(ACTOR),
            fixed_update: case as u64,
            player_state: 2,
            player_position: [0.0; 3],
            accumulated_planar: old,
            additive_position: [0.0; 3],
            stored_intent: [0.0; 2],
            body_velocity: None,
        };
        frame.accumulate_planar_input(input, yaw, INJECTION, DAMPING);
        assert_eq!(
            frame.accumulated_planar.map(f32::to_bits),
            [
                pair.get(true, PLAYER + 0x120),
                pair.get(true, PLAYER + 0x124)
            ],
            "case {case}"
        );
    }
}

struct CandidateServices;
impl GuestCall for CandidateServices {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        assert!(
            matches!(
                address,
                0x42760 | 0x59f90 | 0xd9230 | 0xc5b50 | 0x258ce0 | 0xd2ce0 | 0xd1e20
            ),
            "unexpected helper {address:#x}"
        );
        regs.eax = 0;
    }
}

struct MotionStats;
impl GuestCall for MotionStats {
    fn call(&mut self, address: u32, _regs: &mut ExtRegisters) {
        assert_eq!(address, 0xd1fd0, "unexpected motion helper");
    }
}

#[test]
fn ordinary_258f50_candidate_matches_lifted_body_for_2500_inputs() {
    const PLAYER: u32 = 0x0060_0000;
    const ACTOR: u32 = 0x0061_0000;
    const EXTENSION: u32 = 0x0062_0000;
    const RECORD: u32 = 0x0063_0000;
    const CANDIDATE: u32 = 0x0064_0000;
    const Q4: u32 = 0x0065_0000;
    const Q5: u32 = 0x0066_0000;
    const STACK: u32 = 0x0067_0000;
    let mut pair = locked_pair();
    let mut rng = Rng(0x2394_258f);
    for case in 0..2_500 {
        for base in [PLAYER, ACTOR, EXTENSION, RECORD, CANDIDATE, Q4, Q5, STACK] {
            pair.clear(
                base,
                if base == PLAYER || base == EXTENSION {
                    0x2000
                } else {
                    0x100
                },
            );
        }
        pair.put32(0x007b_0c7c, PLAYER);
        pair.put32(0x007d_e456, 0);
        pair.put32(0x004e_8cec, 1);
        pair.put32(0x004e_794c, 1.0f32.to_bits());
        // The source's scalar cap multiplies P+70+4*k by this global factor.
        pair.put32(0x0047_5c78, 1.0f32.to_bits());
        pair.put32(0x0047_85d4, 0.0f32.to_bits());
        pair.put32(0x0047_8660, 1.01f32.to_bits());
        pair.put32(0x004d_1940, 0);
        pair.put32(PLAYER + 0x04, u32::MAX);
        pair.put32(PLAYER + 0x14, ACTOR);
        pair.put32(PLAYER + 0x114, 0);
        pair.put32(PLAYER + 0x11c, 1);
        // Keep the source's scalar cap outside the generated Q3/Q6 range.
        pair.put32(PLAYER + 0x74, 100_000.0f32.to_bits());
        pair.put32(PLAYER + 0x156c, 0);
        pair.put32(ACTOR + 0x7c, EXTENSION);
        pair.put32(EXTENSION + 0xc00, RECORD);
        pair.put32(RECORD + 8, 0);
        pair.put32(RECORD + 9, 0x20);
        let position = [rng.finite(10_000), rng.finite(10_000), rng.finite(10_000)];
        let accumulated = [rng.finite(8_000), rng.finite(8_000)];
        let additive = [rng.finite(2_000), rng.finite(2_000), rng.finite(2_000)];
        let q3 = rng.finite(2_000);
        let q6 = rng.finite(2_000);
        let basis_q4 = [rng.finite(2_000), rng.finite(2_000)];
        let basis_q5 = [rng.finite(2_000), rng.finite(2_000)];
        for (offset, value) in [0xa8, 0xac, 0xb0].into_iter().zip(position) {
            pair.put32(PLAYER + offset, value.to_bits());
        }
        for (offset, value) in [0x120, 0x124].into_iter().zip(accumulated) {
            pair.put32(PLAYER + offset, value.to_bits());
        }
        for (offset, value) in [0xf4, 0xf8, 0xfc].into_iter().zip(additive) {
            pair.put32(PLAYER + offset, value.to_bits());
        }
        for (index, value) in basis_q4.into_iter().enumerate() {
            pair.put32(Q4 + index as u32 * 4, value.to_bits());
        }
        for (index, value) in basis_q5.into_iter().enumerate() {
            pair.put32(Q5 + index as u32 * 4, value.to_bits());
        }
        pair.put32(STACK, 0);
        // Match the documented six-word 258F50 entry frame: Q1 output at +4,
        // Q3 at +0x0c, Q4/Q5 basis pointers at +0x10/+0x14, and Q6 at +0x18.
        // Q3 and Q6 are generated independently; mode 1 consumes Q3 for the
        // second basis contribution and does not use Q6 for the first term.
        for (offset, value) in [
            (4, CANDIDATE),
            (8, 0x7777_7777),
            (12, q3.to_bits()),
            (16, Q4),
            (20, Q5),
            (24, q6.to_bits()),
        ] {
            pair.put32(STACK + offset, value);
        }
        let mut regs = registers(STACK);
        // The lifted body enters with the current player live in ESI; the
        // state dispatcher supplies it independently of the six stack words.
        regs.esi = PLAYER;
        let mut view = View(pair.rust);
        let mut services = CandidateServices;
        assert!(
            gameplay_player::run(0x258f50, &mut view, &mut services, &mut regs),
            "case {case}; actor output {:?}",
            [
                pair.get(true, ACTOR),
                pair.get(true, ACTOR + 4),
                pair.get(true, ACTOR + 8)
            ]
        );
        let frame = MotionFrame {
            player: PlayerId(PLAYER),
            actor: ActorId(ACTOR),
            fixed_update: case as u64,
            player_state: 0,
            player_position: position,
            accumulated_planar: accumulated,
            additive_position: additive,
            stored_intent: [0.0; 2],
            body_velocity: None,
        };
        let expected = frame.candidate_before_adjustments(CandidatePath::Ordinary {
            movement_mode: 1,
            scalar_q3: q3,
            scalar_q6: q6,
            basis_q4_xz: basis_q4,
            basis_q5_xz: basis_q5,
        });
        assert_eq!(
            [
                pair.get(true, CANDIDATE),
                pair.get(true, CANDIDATE + 4),
                pair.get(true, CANDIDATE + 8)
            ],
            expected.0.map(f32::to_bits),
            "case {case}; position={position:?}, accumulated={accumulated:?}, additive={additive:?}, q3={q3}, q6={q6}, q4={basis_q4:?}, q5={basis_q5:?}"
        );
    }
}
