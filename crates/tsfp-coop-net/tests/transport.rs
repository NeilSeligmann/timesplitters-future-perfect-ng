// SPDX-License-Identifier: GPL-3.0-or-later
use std::time::{Duration, Instant};
use tsfp_coop_net::census::{Impairment, measure, synthetic_input};
use tsfp_coop_net::{Channel, Error, Loopback, MTU};
use tsfp_input::player::{Look, SourceKind};

#[test]
fn two_udp_instances_exchange_frames_and_reliable_events() {
    let sockets = [Loopback::bind().unwrap(), Loopback::bind().unwrap()];
    sockets[0]
        .connect(sockets[1].local_addr().unwrap())
        .unwrap();
    sockets[1]
        .connect(sockets[0].local_addr().unwrap())
        .unwrap();
    let mut channels = [Channel::new(7, 5), Channel::new(7, 5)];
    for (peer, channel) in channels.iter_mut().enumerate() {
        sockets[peer]
            .send(&channel.input(synthetic_input(0, peer as u8)).unwrap())
            .unwrap();
        channel.queue_event(&[peer as u8, 42]).unwrap();
    }
    let start = Instant::now();
    let mut inputs = [false; 2];
    let mut events = [Vec::new(), Vec::new()];
    while start.elapsed() < Duration::from_secs(2) {
        for peer in 0..2 {
            for packet in channels[peer].retransmit(start.elapsed().as_millis() as u64) {
                sockets[peer].send(&packet).unwrap();
            }
            for result in sockets[peer].poll(&mut channels[peer], 64).unwrap() {
                let result = result.unwrap();
                for p in result.inputs {
                    assert_eq!(p.slot, (1 - peer) as u8);
                    assert_eq!(p.source, SourceKind::Network { peer: 0 });
                    inputs[peer] = true;
                }
                events[peer].extend(result.events);
                for ack in result.replies {
                    sockets[peer].send(&ack).unwrap();
                }
            }
        }
        if inputs == [true; 2]
            && events.iter().all(|v| v.len() == 1)
            && channels.iter().all(|c| c.pending_events() == 0)
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(inputs, [true; 2]);
    assert_eq!(events, [vec![vec![1, 42]], vec![vec![0, 42]]]);
    assert!(channels.iter().all(|c| c.pending_events() == 0));
}

#[test]
fn reliable_loss_reorder_duplicates_and_lost_ack() {
    let (mut a, mut b) = (Channel::new(9, 10), Channel::new(9, 10));
    for n in 0..3 {
        a.queue_event(&[n]).unwrap();
    }
    let packets = a.retransmit(0);
    assert!(a.retransmit(9).is_empty());
    assert!(b.receive(&packets[2]).unwrap().events.is_empty());
    assert!(b.receive(&packets[2]).unwrap().events.is_empty());
    assert_eq!(b.receive(&packets[0]).unwrap().events, vec![vec![0]]);
    // Drop packet 1 and every ACK, then retry: event 0 must not repeat.
    let retry = a.retransmit(10);
    assert!(b.receive(&retry[0]).unwrap().events.is_empty());
    let result = b.receive(&retry[1]).unwrap();
    assert_eq!(result.events, vec![vec![1], vec![2]]);
    assert!(b.receive(&retry[2]).unwrap().events.is_empty());
    a.receive(&result.replies[0]).unwrap();
    assert_eq!(a.pending_events(), 0);
    assert!(a.retransmit(20).is_empty());
}

#[test]
fn inputs_recover_two_lost_packets_and_ignore_late_duplicates() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    a.input(synthetic_input(0, 0)).unwrap();
    a.input(synthetic_input(1, 0)).unwrap();
    let packet = a.input(synthetic_input(2, 0)).unwrap();
    assert_eq!(b.receive(&packet).unwrap().inputs.len(), 3);
    assert!(b.receive(&packet).unwrap().inputs.is_empty());
    for step in 0..3 {
        assert_eq!(b.take_input(step).unwrap().step, step);
    }
    assert!(b.take_input(2).is_none());
    assert!(b.receive(&packet).unwrap().inputs.is_empty());
    assert_eq!(
        a.input(synthetic_input(2, 0)),
        Err(Error::NonMonotonicInput)
    );
}

#[test]
fn decoder_rejects_truncation_session_version_nan_and_oversize() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    let packet = a.input(synthetic_input(0, 0)).unwrap();
    for length in 0..packet.len() {
        assert!(b.receive(&packet[..length]).is_err());
    }
    let mut bad = packet.clone();
    bad[0] ^= 1;
    assert_eq!(b.receive(&bad).unwrap_err(), Error::Malformed);
    let mut bad = packet.clone();
    bad[4] ^= 1;
    assert_eq!(b.receive(&bad).unwrap_err(), Error::WrongSession);
    let mut bad = packet.clone();
    bad[36..40].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(b.receive(&bad).unwrap_err(), Error::Malformed);
    assert!(b.receive(&vec![0; MTU + 1]).is_err());
    // Invalid packets did not mutate the input cache.
    assert_eq!(b.receive(&packet).unwrap().inputs.len(), 1);
}

#[test]
fn delta_look_and_buttons_round_trip() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    let mut p = synthetic_input(3, 1);
    p.look = Look::Delta {
        yaw_deg: 12.25,
        pitch_deg: -2.0,
    };
    p.pressed = 0x80;
    p.released = 0x40;
    p.triggers = [0.25, 0.75];
    p.menu.accept = true;
    let decoded = b.receive(&a.input(p).unwrap()).unwrap().inputs[0];
    p.source = SourceKind::Network { peer: 0 };
    assert_eq!(decoded, p);
}

#[test]
fn queues_and_payloads_are_bounded() {
    let mut channel = Channel::new(0, 10);
    assert_eq!(channel.queue_event(&vec![0; MTU]), Err(Error::Malformed));
    for _ in 0..64 {
        channel.queue_event(&[]).unwrap();
    }
    assert_eq!(channel.queue_event(&[]), Err(Error::WindowFull));
    let socket = Loopback::bind().unwrap();
    assert!(socket.connect("192.0.2.1:9".parse().unwrap()).is_err());
    assert!(socket.send(&vec![0; MTU + 1]).is_err());
}

#[test]
fn synthetic_census_measures_first_desync_and_replays_impairment() {
    assert_eq!(measure(600, Impairment::default(), None).first_desync, None);
    let impaired = Impairment {
        loss_percent: 5,
        duplicate_percent: 20,
        latency_ticks: 2,
        jitter_ticks: 3,
        ..Impairment::default()
    };
    let result = measure(600, impaired, None);
    assert_eq!(result, measure(600, impaired, None));
    assert_eq!(result.missing_inputs, [0, 0]);
    assert_eq!(result.first_desync, None);
    assert_eq!(
        measure(600, Impairment::default(), Some(37)).first_desync,
        Some(37)
    );
    let loss = measure(
        600,
        Impairment {
            loss_percent: 100,
            ..Impairment::default()
        },
        None,
    );
    assert_eq!(loss.first_desync, Some(0));
    assert_eq!(loss.missing_inputs, [600, 600]);
}

#[test]
fn malformed_later_frame_does_not_partially_apply_packet() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    a.input(synthetic_input(0, 0)).unwrap();
    let valid = a.input(synthetic_input(1, 0)).unwrap();
    let mut bad = valid.clone();
    *bad.last_mut().unwrap() = 1;
    assert_eq!(b.receive(&bad).unwrap_err(), Error::Malformed);
    assert_eq!(b.receive(&valid).unwrap().inputs.len(), 2);
}

#[test]
fn cumulative_ack_cannot_clear_unsent_events_and_window_recovers() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    for n in 0..64 {
        a.queue_event(&[n]).unwrap();
    }
    let packets = a.retransmit(0);
    for packet in packets.iter().skip(1).rev() {
        assert!(b.receive(packet).unwrap().events.is_empty());
    }
    let received = b.receive(&packets[0]).unwrap();
    assert_eq!(
        received.events,
        (0..64).map(|n| vec![n]).collect::<Vec<_>>()
    );
    let mut invalid_ack = received.replies[0].clone();
    invalid_ack[13..21].copy_from_slice(&65u64.to_le_bytes());
    assert_eq!(a.receive(&invalid_ack).unwrap_err(), Error::Malformed);
    assert_eq!(a.pending_events(), 64);
    a.receive(&received.replies[0]).unwrap();
    assert_eq!(a.queue_event(&[64]), Ok(64));
    // Old ACK cannot clear a newer event.
    a.receive(&received.replies[0]).unwrap();
    assert_eq!(a.pending_events(), 1);
}

#[test]
fn maximum_event_payload_and_bounded_input_history() {
    let (mut a, mut b) = (Channel::new(1, 10), Channel::new(1, 10));
    let payload = vec![42; MTU - 21];
    a.queue_event(&payload).unwrap();
    let packet = a.retransmit(0).remove(0);
    assert_eq!(packet.len(), MTU);
    assert_eq!(b.receive(&packet).unwrap().events, vec![payload]);
    for step in 0..200 {
        b.receive(&a.input(synthetic_input(step, 0)).unwrap())
            .unwrap();
    }
    assert!(b.take_input(0).is_none());
    assert!(b.take_input(135).is_none());
    assert!(b.take_input(136).is_some());
    assert!(b.take_input(199).is_some());
}

#[test]
fn retry_interval_grid_and_saturating_clock() {
    for interval in 0..=32 {
        let mut channel = Channel::new(7, interval);
        channel.queue_event(&[1]).unwrap();
        let effective = interval.max(1);
        assert_eq!(channel.retransmit(100).len(), 1);
        for now in 0..effective {
            assert!(channel.retransmit(100 + now).is_empty());
        }
        assert_eq!(channel.retransmit(100 + effective).len(), 1);
        assert!(channel.retransmit(0).is_empty());
    }
}
