// SPDX-License-Identifier: GPL-3.0-or-later
//! Fabricated wire fixtures only; all payloads are made here, never retail bytes.
use std::cell::Cell;
use std::io::{self, Cursor, Read};
use std::rc::Rc;
use tsfp_scene_oracle::{Limits, Reader, Window, wire};

fn window(frames: u64) -> Window {
    Window {
        first_present: 7,
        frame_count: frames,
    }
}
fn sample() -> Vec<u8> {
    let mut out = Vec::new();
    wire::frame(&mut out, 0);
    wire::call(&mut out, "DrawVertices", &[5, 0, 3]);
    wire::scissor(&mut out, 1, 0, &[[0, 0, 8, 8]]);
    wire::host(&mut out, 1, &[2, 3]);
    wire::memory(&mut out, 0x100, &[1, 2, 3]);
    wire::surface(&mut out, &[0; 8]);
    wire::lib(&mut out, 4, 5);
    wire::texture(&mut out, 1, 1, 1, &[0, 1, 2, 3]);
    wire::present(&mut out, 7);
    wire::present(&mut out, 8);
    out
}
fn reject(bytes: &[u8], frames: u64, limits: Limits, reason: &str) {
    let mut reader = Reader::new(bytes, window(frames), limits).unwrap();
    let error = loop {
        match reader.next_record() {
            Ok(Some(_)) => {}
            Ok(None) => panic!("accepted incomplete capture, expected {reason}"),
            Err(e) => break e,
        }
    };
    assert!(error.reason.contains(reason), "{error}, expected {reason}");
    assert!(
        reader
            .next_record()
            .unwrap_err()
            .reason
            .contains("poisoned")
    );
}

#[test]
fn producer_records_preserve_offsets_order_and_present_numbers() {
    let bytes = sample();
    let expected: Vec<_> = wire::Reader::new(&bytes).map(Result::unwrap).collect();
    let mut reader = Reader::new(bytes.as_slice(), window(2), Limits::default()).unwrap();
    let mut offset = 0;
    for (i, expected_record) in expected.iter().enumerate() {
        let actual = reader.next_record().unwrap().unwrap();
        assert_eq!(actual.record, *expected_record);
        assert_eq!(actual.offset, offset);
        assert_eq!(actual.frame_index, u64::from(i == 9));
        let size = u32::from_le_bytes(
            bytes[offset as usize..offset as usize + 4]
                .try_into()
                .unwrap(),
        );
        offset += u64::from(size) + 4;
    }
    assert_eq!(offset, bytes.len() as u64);
    assert!(reader.next_record().unwrap().is_none());
    assert!(reader.next_record().unwrap().is_none());
}

#[test]
fn every_truncated_prefix_fails_and_poison_is_sticky() {
    let bytes = sample();
    for end in 0..bytes.len() {
        let mut reader = Reader::new(&bytes[..end], window(2), Limits::default()).unwrap();
        loop {
            match reader.next_record() {
                Ok(Some(_)) => {}
                Ok(None) => panic!("accepted prefix ending at {end}"),
                Err(_) => break,
            }
        }
        assert!(reader.next_record().is_err());
    }
}

#[test]
fn omitted_frames_and_unknown_tags_cannot_become_complete_evidence() {
    for (flag, reason) in [
        (wire::FRAME_DRAWS_OMITTED, "draws omitted"),
        (2, "unknown frame flags"),
    ] {
        let mut bytes = Vec::new();
        wire::frame(&mut bytes, flag);
        wire::present(&mut bytes, 7);
        reject(&bytes, 1, Limits::default(), reason);
    }
    reject(
        &[1, 0, 0, 0, 200],
        1,
        Limits::default(),
        "unknown record tag",
    );
}

#[test]
fn trailing_bytes_in_every_known_record_are_rejected() {
    let sample = sample();
    let mut at = 0;
    while at < sample.len() {
        let length = u32::from_le_bytes(sample[at..at + 4].try_into().unwrap()) as usize;
        let mut record = ((length + 1) as u32).to_le_bytes().to_vec();
        record.extend_from_slice(&sample[at + 4..at + 4 + length]);
        record.push(0);
        reject(&record, 1, Limits::default(), "unconsumed record payload");
        at += length + 4;
    }
}

#[test]
fn semantic_payload_sizes_and_address_wrap_reject() {
    let mut bytes = Vec::new();
    wire::texture(&mut bytes, 1, 2, 1, &[0; 4]);
    reject(&bytes, 1, Limits::default(), "texture extent");
    bytes.clear();
    wire::texture(&mut bytes, 1, u32::MAX, u32::MAX, &[]);
    reject(&bytes, 1, Limits::default(), "texture extent");
    bytes.clear();
    wire::scissor(&mut bytes, 2, 0, &[[0; 4]]);
    reject(&bytes, 1, Limits::default(), "scissor count");
    bytes.clear();
    wire::memory(&mut bytes, u32::MAX, &[0; 2]);
    reject(&bytes, 1, Limits::default(), "memory address range wraps");
    bytes.clear();
    wire::call(&mut bytes, "", &[]);
    reject(&bytes, 1, Limits::default(), "empty call name");
}

#[test]
fn receipt_window_rejects_missing_duplicate_unnamed_and_extra_presents() {
    for numbers in [vec![7], vec![7, 7], vec![0, 8], vec![7, 9], vec![7, 8, 9]] {
        let mut bytes = Vec::new();
        for number in &numbers {
            wire::present(&mut bytes, *number);
        }
        let reason = if numbers.len() == 1 {
            "EOF"
        } else if numbers.len() == 3 {
            "after declared"
        } else {
            "expected"
        };
        reject(&bytes, 2, Limits::default(), reason);
    }
    let mut bytes = sample();
    wire::frame(&mut bytes, 0);
    reject(&bytes, 2, Limits::default(), "after declared");
}

struct Metered {
    input: Cursor<Vec<u8>>,
    read: Rc<Cell<usize>>,
    interrupt: bool,
}
impl Read for Metered {
    fn read(&mut self, dest: &mut [u8]) -> io::Result<usize> {
        if self.interrupt {
            self.interrupt = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = self.input.read(dest)?;
        self.read.set(self.read.get() + n);
        Ok(n)
    }
}

#[test]
fn limits_reject_before_payload_read_and_frame_budgets_reset() {
    let consumed = Rc::new(Cell::new(0));
    let source = Metered {
        input: Cursor::new(u32::MAX.to_le_bytes().to_vec()),
        read: consumed.clone(),
        interrupt: true,
    };
    let mut reader = Reader::new(source, window(1), Limits::default()).unwrap();
    assert!(
        reader
            .next_record()
            .unwrap_err()
            .reason
            .contains("record byte limit")
    );
    assert_eq!(consumed.get(), 4);
    let mut bytes = Vec::new();
    wire::frame(&mut bytes, 0);
    wire::present(&mut bytes, 7);
    reject(
        &bytes,
        1,
        Limits {
            frame_bytes: 21,
            ..Limits::default()
        },
        "frame budget",
    );
    reject(
        &bytes,
        1,
        Limits {
            records_per_frame: 1,
            ..Limits::default()
        },
        "frame budget",
    );
    bytes.clear();
    wire::present(&mut bytes, 7);
    wire::present(&mut bytes, 8);
    let mut reader = Reader::new(
        bytes.as_slice(),
        window(2),
        Limits {
            record_bytes: 13,
            frame_bytes: 13,
            records_per_frame: 1,
        },
    )
    .unwrap();
    assert!(reader.next_record().unwrap().is_some());
    assert!(reader.next_record().unwrap().is_some());
    assert!(reader.next_record().unwrap().is_none());
}

#[test]
fn invalid_windows_and_limits_fail_before_reading() {
    for win in [
        Window {
            first_present: 0,
            frame_count: 1,
        },
        Window {
            first_present: 7,
            frame_count: 0,
        },
        Window {
            first_present: u64::MAX,
            frame_count: 2,
        },
    ] {
        assert!(Reader::new(&[][..], win, Limits::default()).is_err());
    }
    for limits in [
        Limits {
            record_bytes: 0,
            ..Limits::default()
        },
        Limits {
            frame_bytes: 0,
            ..Limits::default()
        },
        Limits {
            records_per_frame: 0,
            ..Limits::default()
        },
    ] {
        assert!(Reader::new(&[][..], window(1), limits).is_err());
    }
}
