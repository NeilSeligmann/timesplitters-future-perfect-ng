// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136d: the binary form of the stream (record layout in `tsfp_engine_render::trace_wire`, documented in
//! docs/t-rust-hosted-dispatcher.md). The text grammar of [`crate::stream`] stays the default for dumps and the offline replay
//! tools; this module turns binary records into the same [`Event`]s the text parser produces ([`event_of`]) and converts between
//! the two forms ([`text_to_binary`], [`binary_to_events`]) so the equality can be tested on real captured streams.
use tsfp_engine_render::device::DeviceCall;
use tsfp_engine_render::trace_wire::{self as wire, Reader, Record};

use crate::stream::{Event, Stream, decode_call_words};

/// A record that cannot be turned into an event (unknown call name, short payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryError(pub String);

/// The event a record stands for. `Ok(None)` for a record of an unknown tag (skipped, as the format allows).
pub fn event_of(record: &Record<'_>) -> Result<Option<Event>, BinaryError> {
    Ok(Some(match *record {
        Record::Call { name: "NativeViewLayerBoundary", words } if words.len() == 0 => Event::ViewLayerBoundary,
        Record::Call { name, words } => {
            let call = decode_call_words(name, &words.to_vec(), &[]).ok_or_else(|| {
                BinaryError(format!(
                    "cannot decode call {name} with {} words",
                    words.len()
                ))
            })?;
            Event::Call(call)
        }
        Record::Scissor {
            count,
            flags,
            rects,
        } => Event::Call(DeviceCall::SetScissor {
            count,
            flags,
            rects: (0..rects.len() / 4)
                .map(|at| {
                    [
                        rects.get(at * 4) as i32,
                        rects.get(at * 4 + 1) as i32,
                        rects.get(at * 4 + 2) as i32,
                        rects.get(at * 4 + 3) as i32,
                    ]
                })
                .collect(),
        }),
        Record::Host { function, words } => Event::Host(function, words.to_vec()),
        Record::Memory { address, bytes } => Event::Memory {
            address,
            bytes: bytes.to_vec(),
        },
        Record::Surface { words } => Event::Surface {
            target: words.get(0),
            depth: words.get(1),
            data: words.get(2),
            format: words.get(3),
            size: words.get(4),
            pitch: words.get(5),
            depth_data: words.get(6),
            flags: words.get(7),
        },
        Record::Lib { method, value } => Event::Lib { method, value },
        Record::Present { .. } => Event::Present,
        Record::Texture {
            id,
            width,
            height,
            rgba,
        } => Event::Texture {
            id,
            width,
            height,
            rgba: rgba.to_vec(),
        },
        // flow control for the render thread (rt-host), not a renderer event
        Record::Frame { .. } | Record::Unknown { .. } => return Ok(None),
    }))
}

/// Append the record of one event. A `DeviceCall` goes through its text form to get name and words (the one place the two
/// grammars meet), `SetScissor` is written directly because its corners are signed.
pub fn encode_event(out: &mut Vec<u8>, event: &Event, present_number: u64) -> Result<(), String> {
    match event {
        Event::Call(DeviceCall::SetScissor {
            count,
            flags,
            rects,
        }) => {
            wire::scissor(out, *count, *flags, rects);
        }
        Event::Call(call) => {
            let mut text = Stream {
                events: vec![Event::Call(call.clone())],
            }
            .to_text();
            let line = text.lines().nth(1).ok_or("empty call text")?.to_string();
            text.clear();
            let mut fields = line.split_whitespace();
            fields.next(); // "call"
            let name = fields.next().ok_or("call without a name")?;
            let words = fields
                .map(|field| {
                    u32::from_str_radix(field, 16)
                        .map_err(|_| format!("bad word {field} in {line}"))
                })
                .collect::<Result<Vec<u32>, _>>()?;
            wire::call(out, name, &words);
        }
        Event::Host(function, args) => wire::host(out, *function, args),
        Event::Memory { address, bytes } => wire::memory(out, *address, bytes),
        Event::Texture {
            id,
            width,
            height,
            rgba,
        } => wire::texture(out, *id, *width, *height, rgba),
        Event::Surface {
            target,
            depth,
            data,
            format,
            size,
            pitch,
            depth_data,
            flags,
        } => {
            wire::surface(
                out,
                &[
                    *target,
                    *depth,
                    *data,
                    *format,
                    *size,
                    *pitch,
                    *depth_data,
                    *flags,
                ],
            );
        }
        Event::Lib { method, value } => wire::lib(out, *method, *value),
        Event::Present => wire::present(out, present_number),
        Event::ViewLayerBoundary => wire::call(out, "NativeViewLayerBoundary", &[]),
    }
    Ok(())
}

/// Convert the text form to the binary form, line by line (`# present N` names the next present, other comments are dropped).
pub fn text_to_binary(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pending = 0u64;
    for (number, line) in text.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("# present ") {
            pending = rest.trim().parse().unwrap_or(0);
            continue;
        }
        if let Some(event) = Stream::parse_line(number, line)? {
            let named = if matches!(event, Event::Present) {
                std::mem::take(&mut pending)
            } else {
                0
            };
            encode_event(&mut out, &event, named)?;
        }
    }
    Ok(out)
}

/// Decode a whole buffer into events plus the present numbers (0: the present was not named), for tests and tools.
pub fn binary_to_events(bytes: &[u8]) -> Result<(Vec<Event>, Vec<u64>), String> {
    let (mut events, mut numbers) = (Vec::new(), Vec::new());
    for record in Reader::new(bytes) {
        let record = record.map_err(|offset| format!("malformed record at byte {offset}"))?;
        if let Record::Present { number } = record {
            numbers.push(number);
        }
        if let Some(event) = event_of(&record).map_err(|error| error.0)? {
            events.push(event);
        }
    }
    Ok((events, numbers))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# tsfp-wgpu stream v1\n\
        call PushMethod 40100 7\n\
        call SetScissor 2 1 -4 5 6 7 0 0 640 480\n\
        call SetSurface 1 2 3 4 5 6 7 8\n\
        call SetVertexConstants 4 3f800000 0 0 3f800000\n\
        host 19240 400 24\n\
        mem 1000 00ff10\n\
        mem 2000 -\n\
        lib 1e9c 3\n\
        # a comment\n\
        call DrawVertices 5 0 3\n\
        # present 12\n\
        present\n\
        present\n";

    #[test]
    fn text_and_binary_give_the_same_events() {
        let from_text = Stream::from_text(SAMPLE).unwrap();
        let binary = text_to_binary(SAMPLE).unwrap();
        let (events, numbers) = binary_to_events(&binary).unwrap();
        assert!(!events.is_empty());
        assert_eq!(format!("{:?}", from_text.events), format!("{events:?}"));
        assert_eq!(numbers, vec![12, 0]);
    }

    #[test]
    fn view_boundary_preserves_mixed_draw_and_present_order() {
        let text = "call DrawVertices 8 0 4\nview-layer-boundary\nlib 210 0\npresent\n";
        let stream = Stream::from_text(text).unwrap();
        let binary = text_to_binary(text).unwrap();
        let (events, numbers) = binary_to_events(&binary).unwrap();
        assert_eq!(stream.events, events);
        assert!(matches!(events[1], Event::ViewLayerBoundary));
        assert_eq!(numbers, vec![0]);
        assert_eq!(Stream::from_text(&stream.to_text()).unwrap(), stream);
        assert!(Stream::from_text("view-layer-boundary 1").is_err());
        let mut malformed = Vec::new();
        wire::call(&mut malformed, "NativeViewLayerBoundary", &[1]);
        assert!(binary_to_events(&malformed).is_err());
    }

    #[test]
    fn unknown_tags_are_skipped_and_truncation_is_an_error() {
        let mut bytes = text_to_binary("call InsertFence\n").unwrap();
        bytes.extend_from_slice(&[2, 0, 0, 0, 200, 9]); // tag 200, one payload byte
        let (events, _) = binary_to_events(&bytes).unwrap();
        assert_eq!(events.len(), 1);
        bytes.truncate(bytes.len() - 1);
        assert!(binary_to_events(&bytes).is_err());
    }
}
