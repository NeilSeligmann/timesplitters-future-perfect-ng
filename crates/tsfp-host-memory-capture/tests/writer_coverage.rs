// SPDX-License-Identifier: GPL-3.0-or-later
//! Writer-class admission gate: refuses a purported complete stream when any class lacks a hook or a witness.
use tsfp_host_memory_capture::{
    Writer,
    producer::Producer,
    read,
    writer_coverage::{ALL, ClassOps, HOOKED, Refusal, WriterClass, admit_complete},
};

const HOST: u32 = 30;
const LIFTED: u32 = 31;
const COPY: u32 = 32;
const FILL: u32 = 33;
const ATOMIC: u32 = 34;
const RUST: u32 = 35;
const ALL_OPS: [u32; 6] = [HOST, LIFTED, COPY, FILL, ATOMIC, RUST];
const OPS: [(u32, usize, usize); 6] = [
    (HOST, 2, 0),
    (LIFTED, 2, 0),
    (COPY, 2, 0),
    (FILL, 2, 0),
    (ATOMIC, 2, 0),
    (RUST, 2, 0),
];
const CLASS_OPS: ClassOps = ClassOps {
    host_write: HOST,
    lifted_store: LIFTED,
    guest_copy: COPY,
    guest_fill: FILL,
    guest_atomic: ATOMIC,
    rust_mapped: RUST,
};

fn stream(ops: &[u32]) -> Vec<(u32, Vec<u8>)> {
    let mut t = Producer::new(Writer::new(Vec::new(), 8192).unwrap(), b"cov", true, &OPS).unwrap();
    for op in ops {
        let id = t.begin(1, *op, &[0x1000, 4]).unwrap();
        t.memory(1, true, 0x1000, &[1, 2, 3, 4]).unwrap();
        t.end(1, id, 1, &[]).unwrap();
    }
    read(t.seal().unwrap().as_slice(), 8192).unwrap()
}

#[test]
fn current_census_admits_the_fully_witnessed_stream() {
    // Every class hooked and the five op-id classes witnessed: the stream is admitted.
    assert_eq!(
        admit_complete(&stream(&ALL_OPS), CLASS_OPS, &HOOKED),
        Ok(())
    );
}

#[test]
fn a_census_without_the_direct_pointer_hook_is_refused() {
    let without: Vec<_> = HOOKED
        .iter()
        .copied()
        .filter(|c| *c != WriterClass::DirectHostPointer)
        .collect();
    assert_eq!(
        admit_complete(&stream(&ALL_OPS), CLASS_OPS, &without),
        Err(Refusal::Unhooked(WriterClass::DirectHostPointer))
    );
}

#[test]
fn each_missing_hook_witness_or_id_mutation_is_refused() {
    let full = stream(&ALL_OPS);
    assert_eq!(admit_complete(&full, CLASS_OPS, &ALL), Ok(()));
    // Dropping a hook from the census is refused at that class.
    for class in HOOKED {
        let reduced: Vec<_> = HOOKED.iter().copied().filter(|c| *c != class).collect();
        assert_eq!(
            admit_complete(&full, CLASS_OPS, &reduced),
            Err(Refusal::Unhooked(class))
        );
    }
    // Each hooked class missing its witness event is refused as that class (others present).
    for (skip, class) in [
        (HOST, WriterClass::HostHelper),
        (LIFTED, WriterClass::LiftedStore),
        (COPY, WriterClass::GuestCopy),
        (FILL, WriterClass::GuestFill),
        (ATOMIC, WriterClass::GuestAtomic),
        (RUST, WriterClass::RustMapped),
    ] {
        let ops: Vec<u32> = ALL_OPS.iter().copied().filter(|o| *o != skip).collect();
        assert_eq!(
            admit_complete(&stream(&ops), CLASS_OPS, &ALL),
            Err(Refusal::NoWitness(class))
        );
    }
    // Ambiguous ids (any two classes sharing one op) cannot tell classes apart.
    for same in [
        ClassOps {
            guest_fill: COPY,
            ..CLASS_OPS
        },
        ClassOps {
            guest_atomic: HOST,
            ..CLASS_OPS
        },
        ClassOps {
            lifted_store: ATOMIC,
            ..CLASS_OPS
        },
    ] {
        assert_eq!(
            admit_complete(&full, same, &ALL),
            Err(Refusal::AmbiguousOps)
        );
    }
}
