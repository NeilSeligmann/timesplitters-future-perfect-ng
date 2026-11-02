// SPDX-License-Identifier: GPL-3.0-or-later
use std::io::{self, Write};
use tsfp_host_memory_capture::{Writer, read};

fn capture() -> Vec<u8> {
    let mut out = Writer::new(Vec::new(), 1024).unwrap();
    out.record(1, b"first").unwrap();
    out.record(2, b"second").unwrap();
    out.seal().unwrap()
}

#[test]
fn sealed_stream_preserves_order_and_rejects_mutation_and_truncation() {
    let bytes = capture();
    assert_eq!(
        read(bytes.as_slice(), 1024).unwrap(),
        vec![(1, b"first".to_vec()), (2, b"second".to_vec())]
    );
    for end in 0..bytes.len() {
        assert!(
            read(&bytes[..end], 1024).is_err(),
            "accepted truncation at {end}"
        );
    }
    let mut mutated = bytes.clone();
    mutated[24] ^= 1;
    assert!(read(mutated.as_slice(), 1024).is_err());
    let mut reordered = bytes.clone();
    reordered[8] = 1;
    assert!(read(reordered.as_slice(), 1024).is_err());
    let mut extended = bytes;
    extended.push(0);
    assert!(read(extended.as_slice(), 1024).is_err());
}

#[test]
fn overflow_unsupported_and_write_failure_never_seal() {
    let mut full = Writer::new(Vec::new(), 40).unwrap();
    assert!(full.record(1, b"x").is_err());
    assert!(full.seal().is_err());
    let mut unsupported = Writer::new(Vec::new(), 1024).unwrap();
    unsupported.incomplete();
    assert!(unsupported.record(1, b"x").is_ok());
    assert!(unsupported.seal().is_err());
    let mut reserved = Writer::new(Vec::new(), 1024).unwrap();
    assert!(reserved.record(u32::MAX, b"").is_err());
    assert!(reserved.record(1, b"x").is_err());
    assert!(reserved.seal().is_err());

    struct FailAfterHeader(usize);
    impl Write for FailAfterHeader {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            let n = self.0.min(data.len());
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut broken = Writer::new(FailAfterHeader(8), 1024).unwrap();
    assert!(broken.record(1, b"x").is_err());
    assert!(broken.seal().is_err());
}

#[test]
fn file_publishes_only_sealed_capture_and_never_overwrites() {
    use std::fs;
    use tsfp_host_memory_capture::file::CaptureFile;
    let dir = (0..100)
        .find_map(|index| {
            let path = std::env::temp_dir().join(format!(
                "tsfp-host-memory-capture-{}-{index}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => Some(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => panic!("cannot create test directory: {error}"),
            }
        })
        .expect("no isolated test directory available");
    let final_path = dir.join("capture");
    let partial_path = dir.join("capture.part");
    let mut pending = CaptureFile::create(&final_path, 1024).unwrap();
    pending.record(1, b"event").unwrap();
    assert!(!final_path.exists());
    assert!(partial_path.exists());
    pending.seal().unwrap();
    assert!(!partial_path.exists());
    let saved = fs::read(&final_path).unwrap();
    assert_eq!(
        read(saved.as_slice(), 1024).unwrap(),
        vec![(1, b"event".to_vec())]
    );

    let mut conflict = CaptureFile::create(&final_path, 1024).unwrap();
    conflict.record(2, b"new").unwrap();
    assert!(conflict.seal().is_err());
    assert_eq!(fs::read(&final_path).unwrap(), saved);
    assert!(partial_path.exists());
    fs::remove_file(&partial_path).unwrap();
    fs::remove_file(&final_path).unwrap();
    let mut incomplete = CaptureFile::create(&final_path, 1024).unwrap();
    incomplete.record(1, b"before").unwrap();
    incomplete.incomplete();
    incomplete.record(2, b"after").unwrap();
    assert!(incomplete.seal().is_err());
    assert!(!final_path.exists());
    let partial = fs::read(&partial_path).unwrap();
    assert!(partial.windows(5).any(|bytes| bytes == b"after"));
    assert!(read(partial.as_slice(), 1024).is_err());
    fs::remove_file(&partial_path).unwrap();
    fs::remove_dir(&dir).unwrap();
}
