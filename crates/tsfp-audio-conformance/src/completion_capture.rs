//! Typed offline comparison of captured stream completion operations.
//! No capture is a verdict unless the caller has verified a complete seal.
use tsfp_audio::completion::{Completion, Event};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub stream: u32,
    pub completed: u32,
    pub size: u32,
    pub status_address: u32,
    /// A non-null guest write was attempted and reported success.
    pub completed_ok: Option<bool>,
    pub status_ok: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Begin {
        stream: u32,
        serial: u64,
        ticks: u64,
        max_packets: u32,
        full: bool,
    },
    Push {
        stream: u32,
        serial: u64,
        ticks: u64,
        size: u32,
        completed: u32,
        status_address: u32,
        average: u32,
        frequency: u32,
    },
    Status {
        stream: u32,
        serial: u64,
        ticks: u64,
        max_packets: u32,
        value: u32,
    },
    Pause {
        stream: u32,
        mode: u32,
        ticks: u64,
    },
    Work {
        ticks: u64,
    },
    /// Accepted format transaction; queued abort writes are not captured in v1.
    Format {
        stream: u32,
        serial: u64,
        ticks: u64,
        old_hz: u32,
        new_hz: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub operation: Operation,
    /// Deliveries emitted while this operation ran, in producer order.
    pub deliveries: Vec<Delivery>,
}

/// Diagnostic comparison of typed observations; this alone is not a sealed-capture verdict.
pub fn compare_observations(
    clock_hz: u64,
    dowork: bool,
    observations: &[Observation],
) -> Result<usize, String> {
    if clock_hz == 0 || observations.is_empty() {
        return Err("incomplete capture".into());
    }
    let mut model = Completion::new(clock_hz);
    model.dowork_delivery = dowork;
    let mut delivered = 0;
    for (i, observation) in observations.iter().enumerate() {
        let (ok, events) = match observation.operation {
            Operation::Begin {
                stream,
                serial,
                ticks,
                max_packets,
                full,
            } => {
                let mut events = Vec::new();
                let actual = model
                    .stream_process_begin(stream, serial, ticks, max_packets, &mut events)
                    .map_err(|_| "clock failure")?;
                (actual.is_none() == full, events)
            }
            Operation::Push {
                stream,
                serial,
                ticks,
                size,
                completed,
                status_address,
                average,
                frequency,
            } => (
                model.stream_process_push(
                    stream,
                    serial,
                    ticks,
                    size,
                    completed,
                    status_address,
                    average,
                    frequency,
                ),
                Vec::new(),
            ),
            Operation::Status {
                stream,
                serial,
                ticks,
                max_packets,
                value,
            } => {
                let (actual, events) = model
                    .stream_status(stream, serial, ticks, max_packets)
                    .map_err(|_| "clock failure")?;
                (actual == value, events)
            }
            Operation::Pause {
                stream,
                mode,
                ticks,
            } => (
                true,
                model
                    .pause_note(stream, mode, ticks)
                    .map_err(|_| "clock failure")?,
            ),
            Operation::Work { ticks } => (true, model.work(ticks).map_err(|_| "clock failure")?),
            Operation::Format {
                stream,
                serial,
                ticks,
                old_hz,
                new_hz,
            } => {
                let (accepted, events) = model
                    .stream_format(stream, serial, ticks, old_hz, new_hz, true, &mut || true)
                    .map_err(|_| "clock failure")?;
                if !events.is_empty() {
                    return Err("format abort writes are not captured".into());
                }
                (accepted, events)
            }
        };
        if !ok {
            return Err(format!("operation {i} differs"));
        }
        if events.len() != observation.deliveries.len() {
            return Err(format!("operation {i} delivery count differs"));
        }
        for (event, recorded) in events.iter().zip(&observation.deliveries) {
            if !delivery_equal(event, recorded) {
                return Err(format!(
                    "operation {i} delivery differs or write incomplete"
                ));
            }
            delivered += 1;
        }
    }
    if delivered == 0 {
        return Err("no completion deliveries".into());
    }
    Ok(delivered)
}

fn delivery_equal(event: &Event, recorded: &Delivery) -> bool {
    event.status == 0
        && event.stream == recorded.stream
        && event.completed == recorded.completed
        && event.size == recorded.size
        && event.status_addr == recorded.status_address
        && recorded.completed_ok == (event.completed != 0).then_some(true)
        && recorded.status_ok == (event.status_addr != 0).then_some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_ordered_publication_and_rejects_gaps() {
        let mut rows = vec![
            Observation {
                operation: Operation::Begin {
                    stream: 7,
                    serial: 1,
                    ticks: 0,
                    max_packets: 4,
                    full: false,
                },
                deliveries: vec![],
            },
            Observation {
                operation: Operation::Push {
                    stream: 7,
                    serial: 1,
                    ticks: 0,
                    size: 100,
                    completed: 20,
                    status_address: 24,
                    average: 100,
                    frequency: 0,
                },
                deliveries: vec![],
            },
            Observation {
                operation: Operation::Pause {
                    stream: 7,
                    mode: 0,
                    ticks: 0,
                },
                deliveries: vec![],
            },
            Observation {
                operation: Operation::Status {
                    stream: 7,
                    serial: 1,
                    ticks: 1000,
                    max_packets: 4,
                    value: 1,
                },
                deliveries: vec![Delivery {
                    stream: 7,
                    completed: 20,
                    size: 100,
                    status_address: 24,
                    completed_ok: Some(true),
                    status_ok: Some(true),
                }],
            },
        ];
        assert_eq!(compare_observations(1000, false, &rows), Ok(1));
        assert!(compare_observations(1000, false, &[]).is_err());
        rows[3].deliveries[0].status_address = 25;
        assert!(compare_observations(1000, false, &rows).is_err());
        rows[3].deliveries[0].status_address = 24;
        rows[3].deliveries[0].status_ok = Some(false);
        assert!(compare_observations(1000, false, &rows).is_err());
        rows[3].deliveries.clear();
        assert!(compare_observations(1000, false, &rows).is_err());
    }
}
