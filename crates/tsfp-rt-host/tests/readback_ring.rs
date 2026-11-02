use std::time::{Duration, Instant};
use tsfp_rt_host::{Options, Runtime};
use tsfp_wgpu::live::LiveConfig;

const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");
fn start() -> Runtime {
    let mut live = LiveConfig::software();
    live.output = Some((65, 3));
    live.knobs.push(("pipelined_readback".into(), "1".into()));
    Runtime::start(Options {
        live,
        publish_frames: true,
        quiet: true,
        drop_behind: Some(1),
        drop_queue_bytes: 1,
        ..Default::default()
    })
    .unwrap()
}
#[test]
fn idle_mailbox_delivery_and_shutdown_keep_number_and_size() {
    let rt = start();
    rt.send_line("# present 100");
    rt.write(TITLE.as_bytes());
    rt.flush();
    let deadline = Instant::now() + Duration::from_secs(10);
    let first = loop {
        if let Some(frame) = rt.take_frame() {
            break frame;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    };
    assert_eq!(first.present, 100);
    assert_eq!((first.width, first.height), (65, 3));
    assert!(!first.rgba.is_empty());
    rt.set_output_size(17, 7);
    rt.send_line("# present 101");
    rt.write(TITLE.as_bytes());
    let stats = rt.finish();
    let second = rt
        .take_frame()
        .expect("final callback drained at disconnect");
    assert_eq!(second.present, 101);
    assert_eq!((second.width, second.height), (17, 7));
    assert_eq!(stats.readback_waits, 0);
    assert_eq!(stats.waits, 0);
    assert!(rt.take_frame().is_none());
}
