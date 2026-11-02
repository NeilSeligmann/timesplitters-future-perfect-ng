//! T2210 extension-point contracts which native tests cannot observe during publication.
use tsfp_engine_fileio::async_io::{AsyncIo, CompletionHost, Read, Volume};
use tsfp_engine_fileio::file_object::{BodyHost, File, FileObjects, GDFX, INVALID_PARAMETER};

#[derive(Default)]
struct Completion {
    now: u64,
    order: Vec<&'static str>,
    fail_buffer: bool,
    result: Option<(u32, u32)>,
}
impl CompletionHost for Completion {
    fn now(&self) -> u64 {
        self.now
    }
    fn advance_to(&mut self, ticks: u64) {
        self.now = self.now.max(ticks);
    }
    fn identity(&self, _: u32) -> u32 {
        1
    }
    fn identity_live(&self, _: u32) -> bool {
        true
    }
    fn write_buffer(&mut self, _: u32, bytes: &[u8]) -> bool {
        assert_eq!(bytes, b"payload");
        self.order.push("buffer");
        !self.fail_buffer
    }
    fn write_iosb(&mut self, _: u32, status: u32, information: u32) {
        assert_eq!(self.order, ["buffer"]);
        self.order.push("iosb");
        self.result = Some((status, information));
    }
    fn signal_event(&mut self, _: u32) {
        assert_eq!(self.order, ["buffer", "iosb"]);
        self.order.push("event");
    }
    fn event_signalled(&self, _: u32) -> bool {
        self.order.last() == Some(&"event")
    }
}
#[test]
fn completion_publishes_before_signal_even_when_destination_fails() {
    for fail_buffer in [false, true] {
        let mut host = Completion {
            fail_buffer,
            ..Default::default()
        };
        let mut queue = AsyncIo::default();
        queue.set_enabled(true);
        queue
            .submit(
                &host,
                Read {
                    file: 1,
                    event: 2,
                    iosb: 3,
                    buffer: 4,
                    data: b"payload".to_vec(),
                    requested: 99,
                    volume: Volume::Disc,
                },
            )
            .unwrap();
        assert!(host.order.is_empty());
        assert!(queue.complete_next(&mut host));
        assert_eq!(host.order, ["buffer", "iosb", "event"]);
        assert_eq!(
            host.result,
            Some(if fail_buffer {
                (INVALID_PARAMETER, 0)
            } else {
                (0, 7)
            })
        );
        assert_eq!(queue.stats().bytes, if fail_buffer { 0 } else { 7 });
    }
}

#[derive(Default)]
struct Bodies {
    writes: Vec<(u32, u32)>,
}
impl BodyHost for Bodies {
    fn file(&self, _: u32) -> Option<File> {
        None
    }
    fn allocate(&mut self) -> u32 {
        0
    }
    fn write_body(&mut self, _: u32, _: &[u8; 0x30]) -> bool {
        false
    }
    fn free(&mut self, _: u32) -> bool {
        false
    }
    fn bind(&mut self, _: u32, _: u32, _: u32) -> bool {
        false
    }
    fn find(&self, body: u32) -> Option<u32> {
        (body == 1).then_some(GDFX)
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        self.writes.push((address, value));
        self.writes.len() == 1
    }
    fn reset_heap(&mut self) {}
}
#[test]
fn failed_name_write_retains_the_first_field_and_skips_returned_length() {
    let mut host = Bodies::default();
    let bodies = FileObjects::default();
    assert_eq!(
        bodies.query(&mut host, Some([1, 5, 32, 100, 200])),
        INVALID_PARAMETER
    );
    assert_eq!(host.writes, [(108, 4), (112, GDFX)]);
    assert_eq!(bodies.counters(), [0, 0, 1, 0]);
}
