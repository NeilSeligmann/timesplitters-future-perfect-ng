// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_host_memory_capture::events::{self, Invalid};

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}
fn begin(id: u64, parent: u64, thread: u64, op: u32, args: &[u32]) -> Vec<u8> {
    let mut data = Vec::new();
    for n in [id, parent, thread] {
        data.extend_from_slice(&n.to_le_bytes());
    }
    data.extend(words(&[op, args.len() as u32]));
    data.extend(words(args));
    data
}
fn end(id: u64, result: u32, counts: [u32; 3], output: &[u32]) -> Vec<u8> {
    let mut data = id.to_le_bytes().to_vec();
    data.extend(words(&[
        result,
        counts[0],
        counts[1],
        counts[2],
        output.len() as u32,
    ]));
    data.extend(words(output));
    data
}
fn calls() -> Vec<(u32, Vec<u8>)> {
    let mut bytes = 2u64.to_le_bytes().to_vec();
    bytes.extend(words(&[0x1000, 2]));
    bytes.extend_from_slice(&[0xaa, 0xbb]);
    let mut placement = 2u64.to_le_bytes().to_vec();
    placement.extend(words(&[0]));
    placement.extend_from_slice(&4096u64.to_le_bytes());
    placement.extend(words(&[0, 0x1000]));
    vec![
        (events::IDENTITY, b"build-route".to_vec()),
        (events::EMPTY_REGISTRY, vec![]),
        (events::CALL_BEGIN, begin(1, 0, 9, 90, &[11])),
        (events::CALL_BEGIN, begin(2, 1, 9, 12, &[1, 2])),
        (events::MEMORY_READ, bytes),
        (events::PLACEMENT, placement),
        (events::CALL_END, end(2, 0x1000, [1, 0, 1], &[0])),
        (events::CALL_END, end(1, 0x1000, [0, 0, 0], &[])),
    ]
}

#[test]
fn nested_calls_require_identity_coverage_and_matching_counts() {
    let original = calls();
    let supported = [(90, 1, 0), (12, 2, 1)];
    assert_eq!(
        events::validate(&original, b"build-route", &supported),
        Ok(2)
    );
    assert_eq!(
        events::validate(&original, b"wrong", &supported),
        Err(Invalid::Identity)
    );
    assert_eq!(
        events::validate(&original, b"build-route", &[]),
        Err(Invalid::UnsupportedCall)
    );
    assert_eq!(
        events::validate(&original[..2], b"build-route", &supported),
        Err(Invalid::NoCalls)
    );
    assert_eq!(
        events::validate(&original[..6], b"build-route", &supported),
        Err(Invalid::IncompleteCall)
    );
    let mut broken = original.clone();
    broken[1].1.push(1);
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::InitialState)
    );
    let mut broken = original.clone();
    broken[4].0 = 99;
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::UnknownEvent)
    );
    let mut broken = original.clone();
    broken[4].1.pop();
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::InvalidPayload)
    );
    let mut broken = original.clone();
    broken[4].1[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::InvalidPayload)
    );
    let mut broken = original.clone();
    broken[6].1 = end(2, 0x1000, [0, 0, 1], &[0]);
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::CountMismatch)
    );
    let mut broken = original.clone();
    broken[3].1 = begin(2, 1, 10, 12, &[1, 2]);
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::InterleavedCalls)
    );
    let mut broken = original.clone();
    broken[3].1 = begin(2, 0, 9, 12, &[1, 2]);
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::InterleavedCalls)
    );
    let mut broken = original;
    broken.swap(2, 4);
    assert_eq!(
        events::validate(&broken, b"build-route", &supported),
        Err(Invalid::EventOutsideCall)
    );
}
