// SPDX-License-Identifier: GPL-3.0-or-later
//! T3397 Tier 1: Rust-owned HUD message aging against private lifted `0x14A5A0`.
//! Queue entries, policy and audio answers are FABRICATED; the lifted function and its
//! real queue traversal, timing, cue and retirement code come from TSFP_LIFTED_DIR.
#![cfg(w4_lifted)]

use std::{collections::BTreeMap, ptr};

use tsfp_ui_model::{
    font::{FontObject, FontRecord, FontSet, GlyphRect},
    hud_messages::{QueueCall, replay_owned_native},
    hud_native::{GuestCall, GuestRead, HudState},
    hud_producers::{
        CenterNoticeLayout, CenterNoticeText, CornerNoticeLayout, CornerNoticeText, HudSounds,
        MessagePolicy, OwnedMessageQueue, PickupNoticeLayout, PickupNoticeText,
        ProgressNoticeLayout, ProgressNoticeText, SplitNoticeLayout, SplitNoticeText,
        center_notice, corner_notice, pickup_notice, progress_notice, split_notice,
    },
    messages::Message,
};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
    fn w4_run(va: u32, regs: *mut u32) -> i32;
}

const ARENA_LEN: usize = (1usize << 32) + 0x1_0000;
const STACK: u32 = 0x0030_0000;
const PLAYER_CONTEXT: u32 = 0x0001_0000;
const HUD: u32 = 0x007A_6160;
const MESSAGE_TICK: u32 = 0x0014_A5A0;
const MESSAGE_REPLAY: u32 = 0x0014_AC80;

#[derive(Default)]
struct NativeGuest {
    words: BTreeMap<u32, u32>,
    answers: BTreeMap<u32, u32>,
}

impl GuestRead for NativeGuest {
    fn read32(&self, address: u32) -> Option<u32> {
        Some(self.words.get(&address).copied().unwrap_or_default())
    }
}

impl GuestCall for NativeGuest {
    fn call(&self, address: u32, _args: &[u32]) -> Option<u32> {
        self.answers.get(&address).copied()
    }
}

fn native_title_guest() -> (NativeGuest, FontSet) {
    use tsfp_ui_model::{hud_messages::message_addr as m, hud_prompts::prompt_addr as p};

    let mut guest = NativeGuest::default();
    let put = |guest: &mut NativeGuest, address, value| {
        guest.words.insert(address, value);
    };
    put(&mut guest, m::VIEWPORT_PTR, 0x20000);
    for (offset, value) in [(0, 100), (4, 740), (0xc, 640), (0x14, 0), (0x18, 480)] {
        put(&mut guest, 0x20000 + offset, value);
    }
    put(&mut guest, m::VIEWER, 0x10000);
    put(&mut guest, 0x10004, 0);
    put(&mut guest, 0x10014, 0x11000);
    put(&mut guest, 0x11000 + 0x1cc, 100.0f32.to_bits());
    put(&mut guest, 0x11000 + 0x1d8, 100.0f32.to_bits());
    for (address, value) in [
        (m::MARGIN_LEFT, 7),
        (m::MARGIN_TOP, 7),
        (m::MARGIN_RIGHT, 9),
        (m::MARGIN_BOTTOM, 11),
    ] {
        put(&mut guest, address, value);
    }
    for (address, value) in [
        (p::ALT_TABLE, 0),
        (p::FIXED_WIDTH, 0),
        (p::TEXT_MODE, 0),
        (p::WRAP_SPLIT, 0),
        (p::WRAP_KEEP_SPACES, 0),
        (p::HEIGHT_UNITS, 12),
    ] {
        put(&mut guest, address, value);
    }
    for address in [
        m::SCALE_X,
        m::SCALE_Y,
        p::SCALE_ONE,
        p::TEXT_SCALE_X,
        p::TEXT_SCALE_TOP,
        p::TEXT_SCALE_Y,
        p::HEIGHT_FACTOR,
    ] {
        put(&mut guest, address, 1.0f32.to_bits());
    }
    for table in [
        m::FONTS_TITLE,
        m::FONTS_TITLE_BASE,
        m::FONTS_SINGLE_BASE,
        m::FONTS_PICKUP,
    ] {
        for index in 0..5 {
            put(&mut guest, table + index * 4, 0);
        }
    }
    for font in 0..22 {
        let object = p::FONT_OBJECTS + font * 20;
        put(&mut guest, object, 0);
        put(&mut guest, object + 4, 0);
        put(&mut guest, object + 8, 1.0f32.to_bits());
        put(&mut guest, object + 12, 1.0f32.to_bits());
        put(&mut guest, object + 16, 0);
    }
    put(&mut guest, p::FONT_RECORDS + 0x18, 12.0f32.to_bits());
    guest.answers.insert(m::FN_PLAYERS, 1); // one player
    guest.answers.insert(m::FN_TEXT_MODE, 0);
    put(&mut guest, m::MODE_BYTE & !3, 0);
    put(&mut guest, m::LEVEL_PLAYERS, 1);

    let record = FontRecord {
        cell_width: 8.0,
        rects: vec![
            GlyphRect {
                w: 5,
                h: 10,
                ..Default::default()
            };
            96
        ],
        ..Default::default()
    };
    let fonts = FontSet {
        records: BTreeMap::from([(0, record)]),
        objects: vec![
            FontObject {
                record: 0,
                scale_x: 1.0,
                scale_y: 1.0,
                advance_adjust: 0
            };
            22
        ],
        ..Default::default()
    };
    (guest, fonts)
}

struct Audio {
    cues: Vec<i32>,
    queries: Vec<i32>,
}

impl HudSounds for Audio {
    fn is_playing(&mut self, handle: i32) -> bool {
        self.queries.push(handle);
        false
    }

    fn submit(&mut self, cue: i32) {
        self.cues.push(cue);
    }
}

fn arena() -> *mut u8 {
    let base = unsafe { mmap(ptr::null_mut(), ARENA_LEN, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted guest arena mmap failed");
    unsafe { shim_set_base(base) };
    base
}

fn put32(base: *mut u8, address: u32, value: u32) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 4);
    }
}

fn get32(base: *mut u8, address: u32) -> u32 {
    u32::from_le(unsafe { ptr::read_unaligned(base.add(address as usize).cast()) })
}

fn setup_guest(base: *mut u8, queue: &OwnedMessageQueue, frame_delta: u32, policy: &MessagePolicy) {
    unsafe { ptr::write_bytes(base.add(HUD as usize), 0, 0x177C) };
    put32(base, 0x007B_0C7C, PLAYER_CONTEXT);
    put32(base, PLAYER_CONTEXT + 4, 0);
    put32(base, HUD + 0x30, queue.ring.cursor as u32);
    put32(base, 0x004E_8CEC, 60);
    put32(base, 0x0047_5C78, 1.0f32.to_bits());
    put32(base, 0x004E_794C, 1.0f32.to_bits());
    put32(base, 0x004E_7944, frame_delta);
    for (kind, limit) in policy.limits.iter().enumerate() {
        put32(base, 0x0074_90AC + kind as u32 * 4, *limit as u32);
    }
    for (index, entry) in queue.ring.entries.iter().enumerate() {
        let at = HUD + 0x38 + index as u32 * 0x16C;
        put32(base, at, entry.kind);
        put32(base, at + 0x144, entry.age.to_bits());
        put32(base, at + 0x150, entry.end.to_bits());
        put32(base, at + 0x154, entry.evict_after.to_bits());
        put32(base, at + 0x158, entry.weight as i32 as u32);
        put32(base, at + 0x15C, queue.cues[index].unwrap_or(-1) as u32);
        put32(
            base,
            at + 0x160,
            queue.playing_handles[index].unwrap_or(-1) as u32,
        );
        let text = [b'a' + (index % 26) as u8, 0];
        unsafe { ptr::copy_nonoverlapping(text.as_ptr(), base.add((at + 4) as usize), text.len()) };
    }
}

#[test]
fn owned_message_tick_matches_private_lifted_c_for_512_queues() {
    let base = arena();
    let script = [0u32; 3 * 64];
    let mut policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    // Kind 6 is weightless in the recorded message table, but still ages.

    for seed in 0..512u32 {
        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = seed as usize % 16;
        for (index, entry) in queue.ring.entries.iter_mut().enumerate() {
            *entry = Message {
                kind: ((seed as usize + index) % 14) as u32,
                text: vec![b'a' + (index % 26) as u8, 0],
                age: ((seed as usize + index * 7) % 20) as f32 * 0.1 - 0.4,
                end: 1.5,
                evict_after: 0.8,
                weight: 1 + (index % 3) as u32,
                ..Default::default()
            };
            queue.cues[index] = (index % 3 == 0).then_some(index as i32);
            queue.playing_handles[index] = (index % 4 == 0).then_some(index as i32);
        }
        let frame_delta = u32::from(seed % 7 != 0);
        setup_guest(base, &queue, frame_delta, &policy);
        unsafe {
            shim_script(script.as_ptr(), 64);
        }
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(MESSAGE_TICK, regs.as_mut_ptr()) }, 1);

        let mut native_audio = Audio {
            cues: vec![],
            queries: vec![],
        };
        queue.advance(1.0 / 60.0, frame_delta as i32, &policy, &mut native_audio);
        let mut lifted_cues = Vec::new();
        let mut lifted_queries = Vec::new();
        for call in 0..unsafe { shim_trace_len() } {
            let row = unsafe { shim_trace_entry(call) };
            let va = unsafe { *row };
            let arg = unsafe { *row.add(3) };
            match va {
                0x45540 => lifted_queries.push(arg as i32),
                0x45820 => lifted_cues.push(unsafe { *row.add(13) } as i32),
                _ => {}
            }
        }

        for (index, entry) in queue.ring.entries.iter().enumerate() {
            let at = HUD + 0x38 + index as u32 * 0x16C;
            assert_eq!(
                entry.age.to_bits(),
                get32(base, at + 0x144),
                "seed {seed} entry {index} age"
            );
            assert_eq!(
                entry.kind,
                get32(base, at),
                "seed {seed} entry {index} kind age={} end={} evict={} weight={}",
                entry.age,
                entry.end,
                entry.evict_after,
                entry.weight
            );
            assert_eq!(
                queue.cues[index].unwrap_or(-1) as u32,
                get32(base, at + 0x15C),
                "seed {seed} entry {index} cue"
            );
            assert_eq!(
                queue.playing_handles[index].unwrap_or(-1) as u32,
                get32(base, at + 0x160),
                "seed {seed} entry {index} playback handle"
            );
            assert_eq!(entry.text, vec![b'a' + (index % 26) as u8, 0]);
        }
        assert_eq!(native_audio.queries, lifted_queries, "seed {seed} queries");
        assert_eq!(native_audio.cues, lifted_cues, "seed {seed} cues");
    }
}

#[test]
fn owned_kind7_no_action_replay_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 7,
        text: b"notice\0".to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 3.0,
        evict_after: 2.5,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    put32(base, HUD + 0x1710, 1);
    put32(base, PLAYER_CONTEXT + 0x20, 0); // no input buttons are eligible
    put32(base, 0x0014_AED4 + 6 * 4, 0x0014_AE1D); // kind-7 scheduler branch
    put32(base, STACK + 4, 0); // all message kinds
    let script = [0u32; 3 * 64];
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let lifted_calls: Vec<u32> = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { *shim_trace_entry(index) })
        .collect();
    assert!(lifted_calls.contains(&0x0001_9d70));
    assert!(lifted_calls.contains(&0x0000_427b0));

    let mut progress = 1;
    let mut drawn = Vec::new();
    queue.replay_native(
        &policy,
        false,
        true,
        &mut progress,
        || false,
        |message, _slide, _offset, value| drawn.push((message.kind, value)),
    );
    assert_eq!(drawn, [(7, 1)]);
    assert_eq!(get32(base, HUD + 0x1710), progress);
    assert_eq!(get32(base, HUD + 0x38), queue.ring.entries[0].kind);
    assert_eq!(progress, 1);
    assert_eq!(queue.ring.entries[0].kind, 7);
}

#[test]
fn owned_kind8_corner_notice_dispatch_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 8,
        text: b"corner notice\0".to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 3.0,
        evict_after: 2.5,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    // 0x14AC80 indexes this jump table by kind - 1; kind 8 selects slot 7.
    put32(base, 0x0014_AED4 + 7 * 4, 0x0014_AE93); // kind-8 drawer branch
    put32(base, STACK + 4, 0); // all message kinds
    let script = [0u32; 3 * 64];
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    // 0x12D8E0 is internal to the lifted image; the shim trace exposes its
    // external integer-position text draw instead.
    let corner_draw = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { shim_trace_entry(index) })
        .find(|row| unsafe { **row == 0x0004_2490 })
        .expect("kind-8 drawer branch reaches its integer-position text draw");
    assert_eq!(unsafe { *corner_draw.add(5) }, HUD + 0x3c);

    let mut drawn = Vec::new();
    let mut progress = 0;
    queue.replay_native(
        &policy,
        false,
        true,
        &mut progress,
        || false,
        |message, _slide, _offset, _progress| drawn.push(message.kind),
    );
    assert_eq!(drawn, [8]);
}

#[test]
fn owned_kind8_corner_presenter_matches_private_lifted_draw_args() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 8,
        text: b"corner notice\0".to_vec(),
        age: 3.0,
        fade_start: 2.0,
        end: 4.0,
        evict_after: 4.0,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    let entry = HUD + 0x38;
    unsafe {
        ptr::copy_nonoverlapping(
            queue.ring.entries[0].text.as_ptr(),
            base.add((entry + 4) as usize),
            queue.ring.entries[0].text.len(),
        );
    }
    const VIEWPORT: u32 = 0x0002_0000;
    put32(base, 0x007e_71c8, VIEWPORT);
    put32(base, VIEWPORT, 100);
    put32(base, VIEWPORT + 4, 740);
    put32(base, VIEWPORT + 0x18, 480);
    put32(base, 0x007a_6140, 7);
    put32(base, 0x0074_90ec, 20.0f32.to_bits());
    put32(base, 0x0047_d1f8, 2.0f32.to_bits());
    put32(base, 0x0047_60cc, 3.5f32.to_bits());
    put32(base, 0x0047_5c78, 1.0f32.to_bits());
    put32(base, 0x0047_5cc0, 120.0f32.to_bits());
    put32(base, entry + 0x148, 2.0f32.to_bits());
    put32(base, STACK + 4, entry);

    let mut script = [0u32; 3 * 64];
    script[3] = 7; // 0x12D150 returns a fitting font descriptor.
    script[6] = 60; // 0x3C848C converts the fixture's 0.5 * 120 alpha.
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x0012_D8E0, regs.as_mut_ptr()) }, 1);
    let draw = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { shim_trace_entry(index) })
        .find(|row| unsafe { **row == 0x0004_2490 })
        .expect("lifted kind-8 drawer reaches its integer-position text draw");
    let x = get32(base, unsafe { *draw.add(3) });
    let y = get32(base, unsafe { *draw.add(4) });
    let text_ptr = unsafe { *draw.add(5) };
    let lifted_text = (0..64)
        .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
        .take_while(|byte| *byte != 0)
        .collect::<Vec<_>>();
    let lifted_colour = unsafe { *draw.add(13) };

    struct Text;
    impl CornerNoticeText for Text {
        fn fit_corner_font(&self, payload: &[u8]) -> Option<u32> {
            (payload == b"corner notice").then_some(7)
        }
        fn alpha_integer(&self, value: f64) -> i32 {
            value as i32
        }
    }
    let native = corner_notice(
        &queue.ring.entries[0],
        &CornerNoticeLayout {
            viewport_left: 100,
            margin_left: 7,
            viewport_bottom: 480,
            line_height: 20.0,
            height_factor: 2.0,
            baseline_offset: 3.5,
            full_alpha: 1.0,
            alpha_scale: 120.0,
            colour_base: 0x6464_6400,
        },
        &Text,
    )
    .expect("typed corner presenter accepts the same fabricated message");
    assert_eq!([x as i32, y as i32], native.position);
    assert_eq!(lifted_text, native.text);
    assert_eq!(lifted_colour, native.colour);
}

#[test]
fn owned_kind1_center_presenter_matches_private_lifted_draw_args() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    const ENTRY: u32 = HUD + 0x38;
    const VIEWPORT: u32 = 0x0002_0000;

    struct Text;
    impl CenterNoticeText for Text {
        fn fit_font(&self, payload: &[u8]) -> Option<u32> {
            payload.starts_with(b"center notice").then_some(7)
        }
        fn width(&self, font: u32, payload: &[u8]) -> Option<f64> {
            (font == 7 && payload.starts_with(b"center notice")).then_some(120.0)
        }
        fn bottom_line(&self) -> Option<i32> {
            Some(450)
        }
        fn integer(&self, value: f64) -> i32 {
            value as i32
        }
    }

    // Guest tables, message text and scripted metric/conversion answers are
    // FABRICATED. Cover the documented linear fade lifetime plus line-count
    // placement, keeping viewport meanings and helper answers fixed.
    for seed in 0..18 {
        let age = 2.0 + (seed % 9) as f32 * 0.25;
        let line_count = 1 + seed / 9;
        let payload = if line_count == 1 {
            b"center notice".to_vec()
        } else {
            b"center notice\nsecond line".to_vec()
        };
        let mut text = payload.clone();
        text.push(0);
        let mut queue = OwnedMessageQueue::default();
        queue.ring.entries[0] = Message {
            kind: 1,
            text,
            age,
            fade_start: 2.0,
            end: 4.0,
            evict_after: 4.0,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        unsafe {
            ptr::copy_nonoverlapping(
                queue.ring.entries[0].text.as_ptr(),
                base.add((ENTRY + 4) as usize),
                queue.ring.entries[0].text.len(),
            );
        }
        put32(base, 0x007e_71c8, VIEWPORT);
        put32(base, VIEWPORT, 100);
        put32(base, VIEWPORT + 4, 740);
        put32(base, 0x007a_6140, 7);
        put32(base, 0x007a_6148, 9);
        put32(base, 0x004e_68f8, 1.0f32.to_bits());
        put32(base, 0x0047_5db4, 6.0f32.to_bits());
        put32(base, 0x007e_7264, 20.0f32.to_bits());
        put32(base, 0x0047_5cd4, 0.5f32.to_bits());
        put32(base, 0x0047_5c78, 1.0f32.to_bits());
        put32(base, 0x0047_5cc0, 120.0f32.to_bits());
        put32(base, ENTRY + 0x148, 2.0f32.to_bits());
        put32(base, STACK + 4, ENTRY);

        let alpha = (1.0 - (age - 2.0) / 2.0) * 120.0;
        let mut script = [0u32; 3 * 64];
        script[3] = alpha as u32; // 0x3C848C: linear alpha conversion.
        script[6] = 7; // 0x12D150: fitting-font answer.
        script[9] = 120.0f32.to_bits(); // 0x405C0: fabricated text width.
        script[12] = 359; // 0x3C848C: centered X conversion.
        script[21] = 450; // 0x148680: fabricated bottom line.
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[1] = ENTRY;
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(0x0014_9590, regs.as_mut_ptr()) }, 1);
        let calls: Vec<[u32; 6]> = (0..unsafe { shim_trace_len() })
            .map(|index| {
                let row = unsafe { shim_trace_entry(index) };
                unsafe {
                    [
                        *row,
                        *row.add(1),
                        *row.add(2),
                        *row.add(3),
                        *row.add(4),
                        *row.add(5),
                    ]
                }
            })
            .collect();
        let draw = (0..unsafe { shim_trace_len() })
            .map(|index| unsafe { shim_trace_entry(index) })
            .find(|row| unsafe { **row == 0x0004_1E30 })
            .unwrap_or_else(|| panic!("lifted kind-1 drawer missed 0x41E30; calls={calls:x?}"));
        let lifted_position =
            unsafe { [f32::from_bits(*draw.add(3)), f32::from_bits(*draw.add(4))] };
        let text_ptr = unsafe { *draw.add(5) };
        let lifted_text = (0..64)
            .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        let native = center_notice(
            &queue.ring.entries[0],
            &CenterNoticeLayout {
                viewport_left: 100,
                viewport_right: 740,
                margin_left: 7,
                margin_right: 9,
                scale_y: 1.0,
                bottom_offset: 6.0,
                line_height: 20.0,
                half_width: 0.5,
                full_alpha: 1.0,
                alpha_scale: 120.0,
                // Lifted 0x149590 constructs this with the fixed 0x40646400 base.
                foreground_base: 0x4064_6400,
                outline_base: 0x1e1e_1e00,
            },
            &Text,
        )
        .expect("typed center presenter accepts the same fabricated message");
        assert_eq!(lifted_position, native.position, "seed {seed}");
        assert_eq!(lifted_text, native.text, "seed {seed}");
    }
}

#[test]
fn owned_kind2_pickup_presenter_matches_private_lifted_draw_args() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    const ENTRY: u32 = HUD + 0x38;
    const VIEWPORT: u32 = 0x0002_0000;

    struct Text;
    impl PickupNoticeText for Text {
        fn fit_pickup_font(&self, payload: &[u8]) -> Option<u32> {
            payload.starts_with(b"pickup notice").then_some(7)
        }
        fn line_top(&self, font: u32) -> Option<f32> {
            (font == 7).then_some(20.0)
        }
        fn width(&self, font: u32, payload: &[u8]) -> Option<f32> {
            (font == 7 && payload.starts_with(b"pickup notice")).then_some(120.0)
        }
        fn integer(&self, value: f32) -> i32 {
            value as i32
        }
    }

    // The message, HUD flags, viewport, font choice, line height, bottom line,
    // width and conversion responses are FABRICATED. Vary age over the
    // source-backed fade behavior. Enter through the lifted queue dispatcher so
    // it supplies the pickup drawer's private calling convention correctly.
    for (seed, (age, offset)) in [
        (1.0f32, 0.0f32),
        (2.0, 0.0),
        (2.5, 0.0),
        (3.0, 0.0),
        (3.5, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let payload = b"pickup notice";
        let mut queue = OwnedMessageQueue::default();
        queue.ring.entries[0] = Message {
            kind: 2,
            text: [payload.as_slice(), &[0]].concat(),
            age,
            fade_start: 2.0,
            end: 4.0,
            evict_after: 4.0,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        unsafe {
            ptr::copy_nonoverlapping(
                queue.ring.entries[0].text.as_ptr(),
                base.add((ENTRY + 4) as usize),
                queue.ring.entries[0].text.len(),
            );
        }
        put32(base, HUD + 0x1710, 1); // pickup messages use the one-player table
        put32(base, 0x007e_71c8, VIEWPORT);
        put32(base, VIEWPORT, 100);
        put32(base, VIEWPORT + 4, 740);
        put32(base, VIEWPORT + 0x0c, 640);
        put32(base, VIEWPORT + 0x14, 0);
        put32(base, VIEWPORT + 0x18, 480);
        put32(base, 0x007a_6140, 7);
        put32(base, 0x007a_6144, 9);
        put32(base, 0x007a_6148, 11);
        put32(base, 0x007a_614c, 11);
        put32(base, 0x004e_8cec, 1);
        put32(base, 0x004e_68f4, 1.0f32.to_bits());
        put32(base, 0x004e_68f8, 1.0f32.to_bits());
        put32(base, 0x0047_5cc0, 120.0f32.to_bits());
        put32(base, 0x004e_72a4, 7);
        put32(base, 0x006b_93a8, 0);
        put32(base, 0x007e_72d0, 0);
        let font_object = 0x004c_8de8 + 7 * 20;
        put32(base, font_object, 0);
        put32(base, font_object + 4, 0);
        put32(base, font_object + 8, 1.0f32.to_bits());
        put32(base, font_object + 0xc, 1.0f32.to_bits());
        put32(base, 0x004c_8c28 + 0x18, 20.0f32.to_bits());
        put32(base, 0x004c_0460, 1.0f32.to_bits());
        put32(base, 0x007e_7270, 1.0f32.to_bits());
        put32(base, 0x007e_7274, 1.0f32.to_bits());
        put32(base, 0x007e_7280, 1.0f32.to_bits());
        put32(base, 0x004e_8cf4, 1);
        put32(base, 0x0047_8390, 1.0f32.to_bits());
        put32(base, ENTRY + 0x148, 2.0f32.to_bits());
        put32(base, ENTRY + 0x14c, 0.0f32.to_bits());
        put32(base, 0x0014_aed4 + 4, 0x0014_ad85); // kind-2 pickup drawer branch
        put32(base, STACK + 4, 0); // all message kinds

        let alpha = if age > 2.0 {
            (1.0 - (age - 2.0) / 2.0) * (1.0 - offset).clamp(0.0, 1.0) * 127.0
        } else {
            (1.0 - offset).clamp(0.0, 1.0) * 127.0
        };
        let mut script = [0u32; 3 * 64];
        script[0] = 1; // 0x19D70: outer dispatcher blend-state answer.
        script[6] = 1; // 0x1CDAE0: frame player-count answer.
        script[9] = 1; // 0x1CDAE0: pickup-table player-count answer.
        script[12] = 1; // 0x19D70: pickup drawer blend-state answer.
        script[15] = alpha as u32; // 0x3C848C: pickup alpha conversion.
        script[18] = 7; // 0x12D150: fitting-font answer.
        script[21] = 420; // 0x148680: fabricated bottom line.
        script[24] = 120.0f32.to_bits(); // 0x405C0: fabricated text width.
        script[27] = 358; // 0x3C848C: fabricated centered X conversion.
        script[30] = 120.0f32.to_bits(); // 0x405C0: width used by outlined draw.
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
        let draw = (0..unsafe { shim_trace_len() })
            .map(|index| unsafe { shim_trace_entry(index) })
            .find(|row| unsafe { **row == 0x0004_1e30 })
            .unwrap_or_else(|| {
                let calls: Vec<u32> = (0..unsafe { shim_trace_len() })
                    .map(|index| unsafe { *shim_trace_entry(index) })
                    .collect();
                panic!("seed {seed}: lifted kind-2 drawer missed 0x41E30; calls={calls:x?}")
            });
        let lifted_position =
            unsafe { [f32::from_bits(*draw.add(3)), f32::from_bits(*draw.add(4))] };
        let text_ptr = unsafe { *draw.add(5) };
        let lifted_text = (0..64)
            .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        let lifted_foreground = unsafe { *draw.add(13) };
        let lifted_outline = unsafe { *draw.add(14) };
        let native = pickup_notice(
            &queue.ring.entries[0],
            offset,
            &PickupNoticeLayout {
                center_x: 418,
                bottom_line: 420,
                max_width: 640.0,
                hide_stacked: true,
                alpha_scale: 127.0,
                foreground_base: 0x6464_6400,
                outline_base: 0x1e1e_1e00,
            },
            &Text,
        )
        .expect("typed pickup presenter accepts the same fabricated message");
        assert_eq!(
            lifted_position,
            native.position.map(|value| value as f32),
            "seed {seed}"
        );
        assert_eq!(lifted_text, native.text, "seed {seed}");
        assert_eq!(lifted_foreground, native.foreground, "seed {seed}");
        assert_eq!(lifted_outline, native.outline, "seed {seed}");
    }
}

#[test]
fn owned_kind4_title_presenter_matches_private_lifted_draw_args() {
    owned_title_presenter_matches_private_lifted_draw_args(4);
}

#[test]
fn owned_kind5_title_presenter_matches_private_lifted_draw_args() {
    owned_title_presenter_matches_private_lifted_draw_args(5);
}

fn owned_title_presenter_matches_private_lifted_draw_args(kind: u32) {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let payload = b"A";
    let mut queue = OwnedMessageQueue::default();
    queue.ring.entries[0] = Message {
        kind,
        text: payload.to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 4.0,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    // The lifted replay switch reads the original guest jump table. This FABRICATED
    // fixture supplies the shared kind-4/5 title branch target from sub_0014AC80.
    put32(base, 0x0014_AED4 + (kind - 1) * 4, 0x0014_AEA3);
    const ENTRY: u32 = HUD + 0x38;
    const VIEWPORT: u32 = 0x0002_0000;
    unsafe {
        ptr::copy_nonoverlapping(
            payload.as_ptr(),
            base.add((ENTRY + 4) as usize),
            payload.len(),
        )
    };
    put32(base, ENTRY + 0x148, 2.0f32.to_bits());
    put32(base, ENTRY + 0x14c, 1.0f32.to_bits());
    put32(base, 0x007e_71c8, VIEWPORT);
    for (offset, value) in [(0, 100), (4, 740), (0xc, 640), (0x14, 0), (0x18, 480)] {
        put32(base, VIEWPORT + offset, value);
    }
    for address in [
        0x004e_7278,
        0x004e_727c,
        0x004e_72b4,
        0x004e_728c,
        0x004e_72a0,
    ] {
        for index in 0..5 {
            put32(base, address + index * 4, 0);
        }
    }
    for (address, value) in [
        (0x007a_6140, 7),
        (0x007a_6144, 7),
        (0x007a_6148, 9),
        (0x007a_614c, 11),
        (0x004e_68f4, 1.0f32.to_bits()),
        (0x004e_68f8, 1.0f32.to_bits()),
        (0x0047_5cd4, 1.0f32.to_bits()),
        (0x0047_5d10, 1.0f32.to_bits()),
        (0x004e_8cf4, 12),
        (0x007e_7270, 1.0f32.to_bits()),
        (0x007e_7280, 1.0f32.to_bits()),
        (0x007e_7284, 0),
        (0x007e_72d0, 0),
        (0x004c_8de8, 0),
        (0x004c_8dec, 0),
        (0x004c_8df0, 1.0f32.to_bits()),
        (0x004c_8df4, 1.0f32.to_bits()),
        (0x004c_8df8, 0),
        (0x004c_0478, 12.0f32.to_bits()),
    ] {
        put32(base, address, value);
    }

    let mut script = [0u32; 3 * 64];
    script[0] = 1; // 0x19D70: outer queue blend-state answer.
    script[9] = 0; // 0x19D70: title drawer blend-state answer.
    script[12] = 0; // 0x12D150: fitting font index.
    script[18] = 166; // 0x148830: title line top.
    unsafe { shim_script(script.as_ptr(), 64) };
    // 0x40700 writes the measured integer width through its first output pointer.
    let writes = [5, STACK - 100, 5];
    unsafe { shim_script_writes(writes.as_ptr(), 1) };
    put32(base, STACK + 4, 0);
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let lifted: Vec<_> = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { shim_trace_entry(index) })
        .filter(|row| unsafe { **row == 0x0004_1e30 })
        .collect();
    assert_eq!(lifted.len(), 2, "title draws should meet from both sides");

    let (guest, fonts) = native_title_guest();
    let mut state = HudState::default();
    state.messages[0].ring.entries[0] = queue.ring.entries[0].clone();
    state.messages[0].slides[0] = 1.0;
    let native = match replay_owned_native(&guest, &fonts, 0, &mut state, 0, &policy) {
        QueueCall::Draw(native) => native,
        other => panic!("owned title should render through its native presenter: {other:?}"),
    };
    assert_eq!(native.native_kinds, [kind]);
    assert!(native.original_calls().is_empty());
    assert_eq!(native.calls.len(), lifted.len());
    for (index, (row, call)) in lifted.iter().zip(&native.calls).enumerate() {
        let text_ptr = unsafe { *row.add(5) };
        let lifted_text = (0..16)
            .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        assert_eq!(
            unsafe { [f32::from_bits(*row.add(3)), f32::from_bits(*row.add(4))] },
            [call.x, call.y],
            "position {index}"
        );
        assert_eq!(lifted_text, call.text, "text {index}");
        assert_eq!(unsafe { *row.add(13) }, call.colour, "colour {index}");
        assert_eq!(unsafe { *row.add(14) }, call.outline, "outline {index}");
    }
}

#[test]
fn owned_kind8_corner_presenter_matches_private_lifted_layout_matrix() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    const ENTRY: u32 = HUD + 0x38;
    const VIEWPORT: u32 = 0x0002_0000;

    struct Text;
    impl CornerNoticeText for Text {
        fn fit_corner_font(&self, payload: &[u8]) -> Option<u32> {
            (payload == b"corner notice").then_some(7)
        }
        fn alpha_integer(&self, value: f64) -> i32 {
            value as i32
        }
    }

    // The guest tables, message and helper answers are FABRICATED. Vary the
    // layout inputs and the documented linear fade progression through the
    // measured lifetime, then compare the lifted draw boundary with the typed
    // presenter. The scripted conversion result is the source-formula integer
    // for each age.
    for seed in 0..32i32 {
        let mut queue = OwnedMessageQueue::default();
        queue.ring.entries[0] = Message {
            kind: 8,
            text: b"corner notice\0".to_vec(),
            age: 2.0 + (seed % 9) as f32 * 0.25,
            fade_start: 2.0,
            end: 4.0,
            evict_after: 4.0,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        unsafe {
            ptr::copy_nonoverlapping(
                queue.ring.entries[0].text.as_ptr(),
                base.add((ENTRY + 4) as usize),
                queue.ring.entries[0].text.len(),
            );
        }
        let viewport_left = 100 + seed * 13;
        let viewport_bottom = 360 + seed * 5;
        let margin_left = 7 - seed % 9;
        let line_height = 12.0 + seed as f32 * 0.25;
        let height_factor = 1.25 + (seed % 7) as f32 * 0.1;
        let baseline_offset = 2.0 + (seed % 5) as f32 * 0.75;
        put32(base, 0x007e_71c8, VIEWPORT);
        put32(base, VIEWPORT, viewport_left as u32);
        put32(base, VIEWPORT + 0x18, viewport_bottom as u32);
        put32(base, 0x007a_6140, margin_left as u32);
        put32(base, 0x0074_90ec, line_height.to_bits());
        put32(base, 0x0047_d1f8, height_factor.to_bits());
        put32(base, 0x0047_60cc, baseline_offset.to_bits());
        put32(base, 0x0047_5c78, 1.0f32.to_bits());
        put32(base, 0x0047_5cc0, 120.0f32.to_bits());
        put32(base, ENTRY + 0x148, 2.0f32.to_bits());
        put32(base, STACK + 4, ENTRY);

        let mut script = [0u32; 3 * 64];
        script[3] = 7; // 0x12D150: fitting-font answer.
        script[6] = (120 - (seed % 9) * 15) as u32; // 0x3C848C: source fade * 120.
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(0x0012_D8E0, regs.as_mut_ptr()) }, 1);
        let draw = (0..unsafe { shim_trace_len() })
            .map(|index| unsafe { shim_trace_entry(index) })
            .find(|row| unsafe { **row == 0x0004_2490 })
            .expect("lifted kind-8 drawer reaches its integer-position text draw");
        // The lifted trace preserves the callee's argument words, which are
        // pointers to the caller's stack slots for these by-value integers.
        let lifted_x = get32(base, unsafe { *draw.add(3) }) as i32;
        let lifted_y = get32(base, unsafe { *draw.add(4) }) as i32;
        let text_ptr = unsafe { *draw.add(5) };
        let lifted_text = (0..64)
            .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        let lifted_colour = unsafe { *draw.add(13) };

        let native = corner_notice(
            &queue.ring.entries[0],
            &CornerNoticeLayout {
                viewport_left,
                margin_left,
                viewport_bottom,
                line_height,
                height_factor,
                baseline_offset,
                full_alpha: 1.0,
                alpha_scale: 120.0,
                colour_base: 0x6464_6400,
            },
            &Text,
        )
        .expect("typed corner presenter accepts the same fabricated message");
        assert_eq!([lifted_x, lifted_y], native.position, "seed {seed}");
        assert_eq!(lifted_text, native.text, "seed {seed}");
        assert_eq!(lifted_colour, native.colour, "seed {seed}");
    }
}

#[test]
fn owned_kind9_split_presenter_matches_private_lifted_text_color_and_anchor() {
    const WRAPPED: u32 = 0x0020_0000;
    const VIEWPORT: u32 = 0x0002_0000;
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    struct Text {
        expected_width: f32,
        bottom_line: i32,
    }
    impl SplitNoticeText for Text {
        fn wrap(&self, font: u32, payload: &[u8], width: f32) -> Option<Vec<u8>> {
            assert_eq!(font, 0x15);
            assert_eq!(payload, b"split notice");
            assert_eq!(width, self.expected_width);
            Some(b"wrapped split".to_vec())
        }
        fn bottom_line(&self) -> Option<i32> {
            Some(self.bottom_line)
        }
    }

    // Guest inputs, bottom-line metrics and helper answers are FABRICATED.
    // Keep the one previously validated layout: changing the viewport-right
    // word or left margin changed lifted Y to zero, while Rust stayed at 450.
    // Their ABI meanings need REA evidence before a wider differential is valid.
    let cases = [(100, 740, 7, 9, 0.5)];
    for (seed, (viewport_left, viewport_right, margin_left, margin_right, width_factor)) in
        cases.into_iter().enumerate()
    {
        // Hold the independent vertical metric constant; this matrix varies
        // viewport and wrapping inputs only.
        let bottom_line = 450;
        let width = (viewport_right - viewport_left - margin_left - margin_right) as f32
            * width_factor
            + (viewport_left + margin_left) as f32;

        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = 0;
        queue.ring.entries[0] = Message {
            kind: 9,
            text: b"split notice\0".to_vec(),
            age: 1.0,
            fade_start: 2.0,
            end: 3.0,
            evict_after: 2.5,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        let entry = HUD + 0x38;
        unsafe {
            ptr::copy_nonoverlapping(
                queue.ring.entries[0].text.as_ptr(),
                base.add((entry + 4) as usize),
                queue.ring.entries[0].text.len(),
            );
            ptr::copy_nonoverlapping(b"wrapped split\0".as_ptr(), base.add(WRAPPED as usize), 14);
        }
        put32(base, 0x007e_71c8, VIEWPORT);
        put32(base, VIEWPORT, viewport_left as u32);
        put32(base, VIEWPORT + 4, viewport_right as u32);
        put32(base, 0x007a_6140, margin_left as u32);
        put32(base, 0x007a_6148, margin_right as u32);
        put32(base, 0x0047_5cc8, width_factor.to_bits());

        let mut script = [0u32; 3 * 64];
        script[6] = WRAPPED; // 0x40F00 returns the fabricated wrapped text.
        script[9] = bottom_line as u32; // 0x148680 supplies the fabricated baseline.
        script[12] = 30; // 0x40620 supplies the fabricated measured height.
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[1] = entry;
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(0x0014_9710, regs.as_mut_ptr()) }, 1);
        let call_trace: Vec<[u32; 6]> = (0..unsafe { shim_trace_len() })
            .map(|index| {
                let row = unsafe { shim_trace_entry(index) };
                unsafe {
                    [
                        *row,
                        *row.add(1),
                        *row.add(2),
                        *row.add(3),
                        *row.add(4),
                        *row.add(5),
                    ]
                }
            })
            .collect();
        let draw = (0..unsafe { shim_trace_len() })
            .map(|index| unsafe { shim_trace_entry(index) })
            .find(|row| unsafe { **row == 0x0004_2130 })
            .unwrap_or_else(|| {
                panic!("lifted kind-9 drawer missed 0x42130; calls={call_trace:x?}")
            });
        let lifted_position =
            unsafe { [f32::from_bits(*draw.add(3)), f32::from_bits(*draw.add(4))] };
        let lifted_text_ptr = unsafe { *draw.add(5) };
        let lifted_text = (0..64)
            .map(|offset| unsafe { *base.add(lifted_text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        let lifted_foreground = unsafe { *draw.add(13) };
        let lifted_outline = unsafe { *draw.add(14) };

        let native = split_notice(
            &queue.ring.entries[0],
            &SplitNoticeLayout {
                viewport_left,
                viewport_right,
                margin_left,
                margin_right,
                mode: 0,
                level_players: 1,
                alternate_context: false,
                normal_font: 0x15,
                alternate_font: 0x12,
                width_factor,
                foreground: 0x5f5f_5fff,
                outline: 0x0f0f_0f7f,
            },
            &Text {
                expected_width: width,
                bottom_line,
            },
        )
        .expect("typed split presenter accepts the same fabricated message");
        assert_eq!(lifted_position[0], native.position[0], "seed {seed}");
        assert_eq!(
            lifted_position[1], native.position[1],
            "seed {seed}; calls={call_trace:x?}; viewport={viewport_left}..{viewport_right}; margins={margin_left},{margin_right}; width_factor={width_factor}"
        );
        assert_eq!(lifted_text, native.text, "seed {seed}");
        assert_eq!(lifted_foreground, native.foreground, "seed {seed}");
        assert_eq!(lifted_outline, native.outline, "seed {seed}");
    }
}

#[test]
fn owned_kind9_split_notice_dispatch_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 9,
        text: b"split notice\0".to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 3.0,
        evict_after: 2.5,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    // 0x14AC80 indexes by kind - 1; kind 9 selects slot 8 and valid switch
    // target 0x14AE82, which queries visibility before entering 0x149710.
    put32(base, 0x0014_AED4 + 8 * 4, 0x0014_AE82);
    put32(base, STACK + 4, 0); // all message kinds
    let mut script = [0u32; 3 * 64];
    script[6] = 1; // 0x12CAD0 reports the split notice as visible
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let call_trace: Vec<[u32; 6]> = (0..unsafe { shim_trace_len() })
        .map(|index| {
            let row = unsafe { shim_trace_entry(index) };
            unsafe {
                [
                    *row,
                    *row.add(1),
                    *row.add(2),
                    *row.add(3),
                    *row.add(4),
                    *row.add(5),
                ]
            }
        })
        .collect();
    // 0x149710 is an internal lifted function, so the stub trace records its
    // centered outlined draw (0x42130), not a call whose VA is 0x149710.
    let split_draw = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { shim_trace_entry(index) })
        .find(|row| unsafe { **row == 0x0004_2130 })
        .unwrap_or_else(|| panic!("kind-9 split-notice draw branch missed; calls={call_trace:x?}"));
    assert_eq!(unsafe { *split_draw.add(2) }, 0x0014_97ff);

    let mut drawn = Vec::new();
    let mut progress = 0;
    queue.replay_native(
        &policy,
        false,
        true,
        &mut progress,
        || false,
        |message, _slide, _offset, _progress| drawn.push(message.kind),
    );
    assert_eq!(drawn, [9]);

    // A hidden split notice still contributes scheduling weight even though
    // neither the lifted nor owned path invokes its drawer.
    setup_guest(base, &queue, 1, &policy);
    script[6] = 0; // 0x12CAD0 reports the split notice as hidden
    unsafe { shim_script(script.as_ptr(), 64) };
    regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let trace_len = unsafe { shim_trace_len() };
    assert!((0..trace_len).all(|index| unsafe { *shim_trace_entry(index) != 0x0004_2130 }));
    let weight_query = (0..trace_len)
        .map(|index| unsafe { shim_trace_entry(index) })
        .find(|row| unsafe { **row == 0x0012_df60 })
        .expect("hidden kind-9 notice still reaches scheduling-weight accounting");
    assert_eq!(unsafe { *weight_query.add(7) }, HUD + 0x38); // ECX = entry

    let mut drawn = Vec::new();
    let mut progress = 0;
    let sums = queue.replay_native(
        &policy,
        false,
        false,
        &mut progress,
        || false,
        |message, _slide, _offset, _progress| drawn.push(message.kind),
    );
    assert!(drawn.is_empty());
    assert_eq!(sums[9], 1);
}

#[test]
fn owned_kind6_producer_matches_private_lifted_record_for_fabricated_inputs() {
    let base = arena();
    let tuning = tsfp_ui_model::hud_producers::Kind6MessageTuning {
        fade_start: 3.0,
        end: 5.0,
        evict_after: 3.5,
        cue_scale: 60.0,
    };
    let script = [0u32; 3 * 64];

    for (player, cursor, age, source) in [
        (0u32, 0u32, 0.0f32, 0x1234_5678u32),
        (1, 15, 0.25, 0x8765_4321),
        (2, 7, 1.5, 0x1020_3040),
        (3, 3, -0.5, 0x5566_7788),
    ] {
        let ring = HUD + player * 0x177c;
        unsafe { ptr::write_bytes(base.add(ring as usize), 0, 0x177c) };
        put32(base, ring + 0x30, cursor);
        put32(base, 0x0047_5d18, tuning.fade_start.to_bits());
        put32(base, 0x0047_60cc, tuning.end.to_bits());
        put32(base, 0x0047_d5dc, tuning.evict_after.to_bits());
        put32(base, 0x004e_8cec, tuning.cue_scale as u32);
        put32(base, STACK + 4, player);
        put32(base, STACK + 8, source);
        put32(base, STACK + 12, age.to_bits());
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(0x0014_8dc0, regs.as_mut_ptr()) }, 1);

        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = cursor as usize;
        let slot = cursor as usize;
        // The lifted producer leaves these adjacent queue words untouched.
        queue.cues[slot] = Some(0);
        queue.playing_handles[slot] = Some(0);
        let cue = queue
            .post_kind6(source, age, tuning)
            .expect("non-null opaque source token is preserved");
        let entry = ring + 0x38 + slot as u32 * 0x16c;
        assert_eq!(get32(base, ring + 0x30), queue.ring.cursor as u32);
        assert_eq!(get32(base, entry), queue.ring.entries[slot].kind);
        assert_eq!(get32(base, entry + 4), 0);
        assert_eq!(
            get32(base, entry + 0x144),
            queue.ring.entries[slot].age.to_bits()
        );
        assert_eq!(get32(base, entry + 0x148), tuning.fade_start.to_bits());
        assert_eq!(get32(base, entry + 0x150), tuning.end.to_bits());
        assert_eq!(get32(base, entry + 0x154), tuning.evict_after.to_bits());
        assert_eq!(
            get32(base, entry + 0x158),
            queue.ring.entries[slot].weight as u32
        );
        assert_eq!(get32(base, entry + 0x164), source);
        assert_eq!(get32(base, entry + 0x15c), queue.cues[slot].unwrap() as u32);
        assert_eq!(
            get32(base, entry + 0x160),
            queue.playing_handles[slot].unwrap() as u32
        );
        assert_eq!(cue.event_id, 0x0d6c);
        assert_eq!(cue.parameter, (tuning.cue_scale * age) as i32);
    }

    // Lifted 0x148DC0 returns without advancing the ring for a null source.
    // The same FABRICATED input is rejected by the typed model.
    let ring = HUD;
    unsafe { ptr::write_bytes(base.add(ring as usize), 0, 0x177c) };
    put32(base, ring + 0x30, 9);
    put32(base, STACK + 4, 0);
    put32(base, STACK + 8, 0);
    put32(base, STACK + 12, 0.75f32.to_bits());
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x0014_8dc0, regs.as_mut_ptr()) }, 1);
    assert_eq!(get32(base, ring + 0x30), 9);
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 9;
    assert!(queue.post_kind6(0, 0.75, tuning).is_none());
    assert_eq!(queue.ring.cursor, 9);
}

#[test]
fn owned_kind6_noop_dispatch_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 6,
        text: b"objective event\0".to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 3.0,
        evict_after: 2.5,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    // The lifted dispatcher indexes by kind - 1; its kind-6 label is the
    // scheduler's measured no-op branch (T2329 checkpoint 9).
    put32(base, 0x0014_AED4 + 5 * 4, 0x0014_AEBF);
    put32(base, STACK + 4, 0); // all message kinds
    let script = [0u32; 3 * 64];
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let lifted_calls: Vec<u32> = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { *shim_trace_entry(index) })
        .collect();
    assert!(!lifted_calls.iter().any(|va| {
        matches!(
            va,
            0x0014_9590 | 0x0014_9810 | 0x0014_9710 | 0x0012_D8E0 | 0x0012_DF60
        )
    }));

    let mut drawn = Vec::new();
    let mut progress = 0;
    let sums = queue.replay_native(
        &policy,
        false,
        true,
        &mut progress,
        || false,
        |message, _slide, _offset, _progress| drawn.push(message.kind),
    );
    assert!(drawn.is_empty());
    assert!(sums.iter().all(|sum| *sum == 0));
}

#[test]
fn owned_kind7_action_retires_at_three_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 0;
    queue.ring.entries[0] = Message {
        kind: 7,
        text: b"notice\0".to_vec(),
        age: 0.5,
        fade_start: 2.0,
        end: 3.0,
        evict_after: 2.5,
        weight: 1,
        ..Default::default()
    };
    setup_guest(base, &queue, 1, &policy);
    put32(base, HUD + 0x1710, 2);
    put32(base, PLAYER_CONTEXT + 0x20, 1);
    put32(base, 0x0014_AED4 + 6 * 4, 0x0014_AE1D);
    put32(base, STACK + 4, 0);
    let mut script = [0u32; 3 * 64];
    script[6] = 1; // the third guest call, 0x427B0, reports an eligible button
    unsafe { shim_script(script.as_ptr(), 64) };
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
    let lifted_calls: Vec<u32> = (0..unsafe { shim_trace_len() })
        .map(|index| unsafe { *shim_trace_entry(index) })
        .collect();
    assert!(lifted_calls.contains(&0x0000_427b0));
    assert!(lifted_calls.contains(&0x0004_2870));
    // 0x12D9C0 is internal to lifted 0x14AC80, so it is not in this trace.

    let mut state = HudState::default();
    state.messages[0] = queue;
    state.input_progress[0] = 2;
    assert!(state.publish_message_buttons(0, 1, 1));
    let mut drawn = Vec::new();
    state
        .replay_messages_native(0, &policy, false, true, |message, _, _, value| {
            drawn.push((message.kind, value));
        })
        .unwrap();
    assert_eq!(drawn, [(7, 3)]);
    assert_eq!(get32(base, HUD + 0x1710), state.input_progress[0]);
    assert_eq!(
        get32(base, HUD + 0x38),
        state.messages[0].ring.entries[0].kind
    );
    assert_eq!(state.input_progress[0], 0);
    assert_eq!(state.messages[0].ring.entries[0].kind, 0);
}

#[test]
fn owned_kind7_progress_boundary_matrix_matches_private_lifted_c() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };

    // The progress counter, button eligibility and message are FABRICATED;
    // compare retirement and final counter state with the private lifted
    // scheduler at the exact documented boundary, including wrapping input.
    for (initial, eligible) in [
        (0, false),
        (0, true),
        (1, false),
        (1, true),
        (2, false),
        (2, true),
        (3, false),
        (3, true),
        (u32::MAX, true),
    ] {
        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = 0;
        queue.ring.entries[0] = Message {
            kind: 7,
            text: b"notice\0".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 3.0,
            evict_after: 2.5,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        put32(base, HUD + 0x1710, initial);
        put32(base, PLAYER_CONTEXT + 0x20, u32::from(eligible));
        put32(base, 0x0014_AED4 + 6 * 4, 0x0014_AE1D);
        put32(base, STACK + 4, 0);
        let mut script = [0u32; 3 * 64];
        if eligible {
            script[6] = 1; // 0x427B0 reports an eligible action.
        }
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(MESSAGE_REPLAY, regs.as_mut_ptr()) }, 1);
        let lifted_progress = get32(base, HUD + 0x1710);
        let lifted_kind = get32(base, HUD + 0x38);

        let mut state = HudState::default();
        state.messages[0] = queue;
        state.input_progress[0] = initial;
        assert!(state.publish_message_buttons(0, u32::from(eligible), 1));
        state
            .replay_messages_native(0, &policy, false, true, |_, _, _, _| {})
            .unwrap();

        assert_eq!(
            state.input_progress[0], lifted_progress,
            "initial={initial}, eligible={eligible}"
        );
        assert_eq!(
            state.messages[0].ring.entries[0].kind, lifted_kind,
            "initial={initial}, eligible={eligible}"
        );
    }
}

#[test]
fn owned_kind7_progress_presenter_matches_private_lifted_draw_args() {
    let base = arena();
    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    const ENTRY: u32 = HUD + 0x38;
    const VIEWPORT: u32 = 0x0002_0000;

    struct Text;
    impl ProgressNoticeText for Text {
        fn count(&self, text: &[u8]) -> Option<u32> {
            Some(text.len() as u32)
        }
        fn prefix_offset(&self, text: &[u8], count: u32) -> Option<usize> {
            (count as usize <= text.len()).then_some(count as usize)
        }
        fn fit_font(&self, text: &[u8]) -> Option<u32> {
            (!text.is_empty()).then_some(7)
        }
        fn wrap(&self, text: &[u8], width: f32) -> Option<Vec<u8>> {
            (width > 0.0).then(|| text.to_vec())
        }
        fn alpha_integer(&self, value: f64) -> i32 {
            value as i32
        }
    }

    // The payload, viewport and helper answers are FABRICATED. Progress two
    // requests the full encoded text; varying age exercises source-backed fade.
    for (seed, age) in [0.5f32, 3.0, 3.5].into_iter().enumerate() {
        let mut payload = b"progress notice".to_vec();
        payload.push(0);
        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = 0;
        queue.ring.entries[0] = Message {
            kind: 7,
            text: payload.clone(),
            age,
            fade_start: 2.0,
            end: 4.0,
            evict_after: 4.0,
            weight: 1,
            ..Default::default()
        };
        setup_guest(base, &queue, 1, &policy);
        unsafe {
            ptr::copy_nonoverlapping(
                payload.as_ptr(),
                base.add((ENTRY + 4) as usize),
                payload.len(),
            );
        }
        put32(base, HUD + 0x1710, 2);
        put32(base, STACK + 4, ENTRY);
        put32(base, 0x007e_71c8, VIEWPORT);
        put32(base, VIEWPORT, 100);
        put32(base, VIEWPORT + 4, 740);
        put32(base, VIEWPORT + 0x14, 480);
        put32(base, 0x007a_6140, 7);
        // Frame::read maps 0x7A6144 to `top` (used as the lower-origin
        // progress baseline) and 0x7A6148 to `right`.
        put32(base, 0x007a_6144, 9);
        put32(base, 0x007a_6148, 11);
        put32(base, 0x004e_68f4, 1.0f32.to_bits());
        put32(base, 0x004e_68f8, 1.0f32.to_bits());
        put32(base, 0x0047_82c4, 2.0f32.to_bits());
        put32(base, 0x0047_5c78, 1.0f32.to_bits());
        put32(base, 0x0047_5cc0, 120.0f32.to_bits());
        put32(base, 0x006b_93a8, 1);

        // Run the lifted presenter directly, as in the kind-8/9 geometry
        // differentials. Its metrics and helper answers are FABRICATED.
        let mut script = [0u32; 3 * 64];
        script[0] = 1;
        let alpha = if age > 2.0 {
            (1.0 - (age - 2.0) / 2.0) * 120.0
        } else {
            120.0
        };
        script[3] = payload.len() as u32; // 0x40650: full encoded text count.
        script[6] = ENTRY + 4 + payload.len() as u32; // 0x406A0: prefix offset.
        script[9] = alpha as u32; // 0x3C848C: linear alpha conversion.
        script[12] = 7; // 0x12D150: fitting-font answer.
        script[15] = 1; // 0x1CDAE0: one wrapped line.
        script[18] = ENTRY + 4; // 0x40F00: preserve the bounded fixture text.
        unsafe { shim_script(script.as_ptr(), 64) };
        let mut regs = [0u32; 8];
        regs[1] = ENTRY;
        regs[7] = STACK;
        assert_eq!(unsafe { w4_run(0x0012_D9C0, regs.as_mut_ptr()) }, 1);
        let draw = (0..unsafe { shim_trace_len() })
            .map(|index| unsafe { shim_trace_entry(index) })
            .find(|row| unsafe { **row == 0x0004_1e30 })
            .unwrap_or_else(|| {
                let calls: Vec<u32> = (0..unsafe { shim_trace_len() })
                    .map(|index| unsafe { *shim_trace_entry(index) })
                    .collect();
                panic!("seed {seed}: lifted kind-7 drawer missed 0x41E30; calls={calls:x?}")
            });
        let lifted_position =
            unsafe { [f32::from_bits(*draw.add(3)), f32::from_bits(*draw.add(4))] };
        let text_ptr = unsafe { *draw.add(5) };
        let lifted_text = (0..64)
            .map(|offset| unsafe { *base.add(text_ptr.wrapping_add(offset) as usize) })
            .take_while(|byte| *byte != 0)
            .collect::<Vec<_>>();
        let lifted_foreground = unsafe { *draw.add(13) };
        let lifted_outline = unsafe { *draw.add(14) };
        let native = progress_notice(
            &queue.ring.entries[0],
            2,
            &ProgressNoticeLayout {
                viewport_left: 100,
                viewport_right: 740,
                viewport_bottom: 480,
                margin_left: 7,
                margin_right: 11,
                margin_bottom: 9,
                effective_players: 1,
                scale: [1.0, 1.0],
                reveal_rate: 2.0,
                full_alpha: 1.0,
                alpha_scale: 120.0,
                foreground_base: 0x6464_6400,
                outline_base: 0x1e1e_1e00,
            },
            &Text,
        )
        .expect("typed progress presenter accepts the same fabricated message");
        assert_eq!(lifted_position, native.position, "seed {seed}");
        assert_eq!(lifted_text, native.text, "seed {seed}");
        assert_eq!(lifted_foreground, native.foreground, "seed {seed}");
        assert_eq!(lifted_outline, native.outline, "seed {seed}");
    }
}
