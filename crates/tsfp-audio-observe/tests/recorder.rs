use std::io::{self, Write};
use tsfp_audio::contract::Command;
use tsfp_audio_observe::{MAX_LINE_BYTES, Recorder};

#[test]
fn failed_records_cannot_be_sealed_or_resumed() {
    let mut out = Vec::new();
    let mut recorder = Recorder::new(&mut out, 48000, 48000).unwrap();
    let huge = Command::SubmitAdpcm {
        key: 1,
        ticks: 0,
        source_hz: 48000,
        mono: true,
        data: vec![0; MAX_LINE_BYTES / 2],
    };
    assert_eq!(
        recorder
            .command(&huge, false, None)
            .unwrap_err()
            .to_string(),
        "record exceeds bound"
    );
    assert!(
        recorder
            .command(&Command::ResetBuffers, true, None)
            .is_err()
    );
    assert!(recorder.finish().is_err());
    assert!(!out.windows(4).any(|w| w == b"END "));
    assert!(out.len() <= MAX_LINE_BYTES + 64);
}

#[test]
fn sink_errors_and_short_writes_are_not_hidden() {
    struct Sink {
        bytes: Vec<u8>,
        remaining: usize,
        flush_fail: bool,
    }
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("injected disk failure"));
            }
            let n = b.len().min(self.remaining).min(3);
            self.bytes.extend_from_slice(&b[..n]);
            self.remaining -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.flush_fail {
                Err(io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }
    let sink = Sink {
        bytes: Vec::new(),
        remaining: 40,
        flush_fail: false,
    };
    let mut recorder = Recorder::new(sink, 48000, 48000).unwrap();
    assert!(recorder.render(100, 1, &[10, -10]).is_err());
    assert!(recorder.finish().is_err());
    let sink = Sink {
        bytes: Vec::new(),
        remaining: usize::MAX,
        flush_fail: true,
    };
    assert!(Recorder::new(sink, 48000, 48000).unwrap().finish().is_err());
    let sink = Sink {
        bytes: Vec::new(),
        remaining: usize::MAX,
        flush_fail: false,
    };
    let short = Recorder::new(sink, 48000, 48000)
        .unwrap()
        .finish()
        .unwrap()
        .bytes;
    let normal = Recorder::new(Vec::new(), 48000, 48000)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(short, normal);
}

#[test]
fn invalid_shapes_and_unrecordable_verdicts_fail_closed() {
    for (capacity, pcm) in [(0, vec![1, 2]), (1, vec![1]), (1_048_577, vec![])] {
        let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
        assert!(recorder.render(1, capacity, &pcm).is_err());
        assert!(recorder.finish().is_err());
    }
    let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
    assert!(
        recorder
            .command(&Command::ResetBuffers, false, None)
            .is_err()
    );
    assert!(recorder.finish().is_err());
    let mut recorder = Recorder::new(Vec::new(), 48000, 48000).unwrap();
    assert!(
        recorder
            .command(&Command::ResetBuffers, true, Some(&[0]))
            .is_err()
    );
    assert!(Recorder::new(Vec::new(), 0, 48000).is_err());
    assert!(Recorder::new(Vec::new(), 48000, 0).is_err());
}
