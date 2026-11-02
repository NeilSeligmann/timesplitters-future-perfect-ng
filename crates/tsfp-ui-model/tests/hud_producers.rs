//! Tier 1: FABRICATED finite inputs versus the lifted-C-to-Rust updater.
#[path = "support/pause_memory.rs"]
mod pause_memory;
use pause_memory::Mem;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, Xmm},
    memory::GuestMemory,
};
use tsfp_ui_model::hud_producers::*;

#[test]
fn kind6_producer_owns_ring_record_and_cue_request() {
    let mut queue = OwnedMessageQueue::default();
    queue.cues[15] = Some(0);
    queue.playing_handles[15] = Some(0);
    queue.ring.cursor = 15;
    queue.ring.entries[15] = tsfp_ui_model::messages::Message {
        kind: 3,
        text: b"old".to_vec(),
        ..Default::default()
    };
    let tuning = Kind6MessageTuning {
        fade_start: 3.0,
        end: 5.0,
        evict_after: 3.5,
        cue_scale: 100.0,
    };
    let cue = queue.post_kind6(0x1234, 0.25, tuning).unwrap();
    assert_eq!(cue.event_id, 0x0d6c);
    assert_eq!(cue.parameter, 25);
    assert_eq!(queue.ring.cursor, 0);
    let entry = &queue.ring.entries[15];
    assert_eq!(entry.kind, 6);
    assert!(entry.text.is_empty());
    assert_eq!(entry.age, -0.25);
    assert_eq!(entry.fade_start, 3.0);
    assert_eq!(entry.end, 5.0);
    assert_eq!(entry.evict_after, 3.5);
    assert_eq!(entry.weight, 1);
    assert_eq!(queue.cues[15], Some(0));
    assert_eq!(queue.playing_handles[15], Some(0));
    assert_eq!(queue.progress[15], 0x1234);

    let before = queue.clone();
    assert_eq!(queue.post_kind6(0, 1.0, tuning), None);
    assert_eq!(queue, before);
}

#[test]
fn explicit_context_show_preserves_cycle_alpha_and_active_slide() {
    for phase in 0..4 {
        let mut animation = WeaponAnimation {
            phase,
            flash: 0.5,
            alpha: 0.75,
            slide: 2.0,
            target: 3.0,
            cycle: -0.5,
        };
        animation.apply_event(WeaponPanelEvent::Show, 1.0);
        assert_eq!(animation.phase, 1);
        assert_eq!(animation.flash, 0.0);
        assert_eq!(animation.cycle, -0.5);
        assert_eq!(animation.alpha, 0.75);
        assert_eq!(animation.slide, if phase == 0 { 0.0 } else { 2.0 });
        assert_eq!(animation.target, if phase == 0 { 0.0 } else { 3.0 });
    }
}

#[test]
fn weapon_animation_matches_lifted_for_4096_states() {
    let mut seed = 0x12345678u32;
    for i in 0..4096 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 2000) as f32 / 100.0 - 10.0
        };
        let mut s = WeaponAnimation {
            phase: i % 5,
            flash: next(),
            alpha: next(),
            slide: next(),
            target: next(),
            cycle: next(),
        };
        let k = WeaponAnimationTuning {
            full: 1.0,
            fade_rate: 4.0,
            cycle_rate: 2.0,
            slide_epsilon: 0.01,
            slide_max_step: 0.1,
            slide_gain: 8.0,
        };
        let t = HudTiming {
            rate_hz: [30, 50, 60, 120][i as usize % 4],
            scale: 0.5 + (i % 5) as f32 * 0.5,
            frames: (i % 4) as f32,
        };
        let ending = i % 7 == 0;
        let mut m = Mem::default();
        m.write32(0x7b0c7c, 0x10000);
        m.write32(0x10004, 0);
        m.write32(0x7de30c, if ending { 8 } else { 7 });
        m.write32(0x4e8cec, t.rate_hz as u32);
        for (va, v) in [
            (0x475c78, k.full),
            (0x475db4, k.fade_rate),
            (0x475d10, k.cycle_rate),
            (0x475d4c, k.slide_epsilon),
            (0x475d40, k.slide_max_step),
            (0x47862c, k.slide_gain),
            (0x4e794c, t.scale),
            (0x4e7948, t.frames),
        ] {
            m.write32(va, v.to_bits());
        }
        m.write32(0x7a78b0, s.phase);
        for (off, v) in [
            (0x1754, s.flash),
            (0x1758, s.alpha),
            (0x175c, s.slide),
            (0x1760, s.target),
            (0x1764, s.cycle),
        ] {
            m.write32(0x7a6160 + off, v.to_bits());
        }
        let mut r = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x9000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        tsfp_ports::c2r::ui_hud::game_hud_player_slot_7a6160_field_0x1750_fade_state_advance(
            &mut m, &mut r,
        );
        s.advance(t, k, ending);
        assert_eq!(s.phase, m.read32(0x7a78b0), "phase seed {i}");
        for (off, v) in [
            (0x1754, s.flash),
            (0x1758, s.alpha),
            (0x175c, s.slide),
            (0x1760, s.target),
            (0x1764, s.cycle),
        ] {
            assert_eq!(
                v.to_bits(),
                m.read32(0x7a6160 + off),
                "seed {i} field {off:x}"
            );
        }
    }
}

use tsfp_ports::abi::GuestCall;
struct Audio {
    mem: Mem,
    policy: MessagePolicy,
    cues: Vec<i32>,
    queries: Vec<i32>,
}
impl HudSounds for Audio {
    fn is_playing(&mut self, h: i32) -> bool {
        self.queries.push(h);
        h % 2 == 0
    }
    fn submit(&mut self, cue: i32) {
        self.cues.push(cue);
    }
}
impl GuestCall for Audio {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let arg = self.mem.read32(r.esp);
        match va {
            0x12df00 => {
                let kind = self.mem.read32(arg) as usize;
                let weight = self.mem.read32(arg + 0x158) as i32;
                let counts = self.mem.read32(r.esp + 4);
                let sum = (self.mem.read32(counts + kind as u32 * 4) as i32).wrapping_add(weight);
                r.eax = u32::from(
                    sum > self.policy.limits[kind]
                        || (1..14).any(|other| {
                            other != kind
                                && self.mem.read32(counts + other as u32 * 4) != 0
                                && (self.policy.conflicts.contains(&(other, kind))
                                    || self.policy.conflicts.contains(&(kind, other)))
                        }),
                );
            }
            0x45540 => r.eax = u32::from(self.is_playing(arg as i32)),
            0x45390 => {} // submit reads only the cue id written by the producer
            0x45820 => self.submit(self.mem.read32(arg) as i32),
            _ => panic!("unexpected {va:x}"),
        }
    }
}

/// Tier 0 cross-check of FABRICATED queues against the mechanically translated
/// `0x14A5A0`; the private lifted-C differential lives in `tsfp-c-diff`.
#[test]
fn owned_message_lifecycle_matches_c2r_for_512_queues() {
    for seed in 0..512u32 {
        let mut queue = OwnedMessageQueue::default();
        queue.ring.cursor = seed as usize % 16;
        let policy = MessagePolicy {
            limits: [4; 14],
            conflicts: vec![(2, 4), (4, 5), (5, 2)],
        };
        for (i, e) in queue.ring.entries.iter_mut().enumerate() {
            *e = tsfp_ui_model::messages::Message {
                kind: ((seed as usize + i) % 14) as u32,
                text: vec![(b'a' + (i % 26) as u8), 0],
                age: ((seed as usize + i * 7) % 20) as f32 * 0.1 - 0.4,
                end: 1.5,
                evict_after: 0.8,
                weight: 1 + (i % 3) as u32,
                ..Default::default()
            };
            queue.cues[i] = (i % 3 == 0).then_some(i as i32);
            queue.playing_handles[i] = (i % 4 == 0).then_some(i as i32);
        }
        let mut m = Mem::default();
        m.write32(0x7b0c7c, 0x10000);
        m.write32(0x10004, 0);
        m.write32(0x4e8cec, 60);
        m.write32(0x475c78, 1f32.to_bits());
        m.write32(0x4e794c, 1f32.to_bits());
        let delta = if seed % 7 == 0 { 0 } else { 1 };
        m.write32(0x4e7944, delta);
        m.write32(0x7a6190, queue.ring.cursor as u32);
        for kind in 0..14 {
            m.write32(0x7490ac + 4 * kind, policy.limits[kind as usize] as u32);
        }
        for (i, e) in queue.ring.entries.iter().enumerate() {
            let at = 0x7a6198 + i as u32 * 0x16c;
            m.write32(at, e.kind as u32);
            for (off, v) in [(0x144, e.age), (0x150, e.end), (0x154, e.evict_after)] {
                m.write32(at + off, v.to_bits());
            }
            m.write32(at + 0x158, e.weight as i32 as u32);
            m.write32(at + 0x15c, queue.cues[i].unwrap_or(-1) as u32);
            m.write32(at + 0x160, queue.playing_handles[i].unwrap_or(-1) as u32);
        }
        let mut lifted_audio = Audio {
            mem: m.clone(),
            policy: policy.clone(),
            cues: vec![],
            queries: vec![],
        };
        let mut r = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x9000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        tsfp_ports::c2r::ui_hud::game_hud_per_frame_sound_cue_for_slot_counts_over_limit_unless_handle_already_playing(&mut m,&mut lifted_audio,&mut r);
        let mut native_audio = Audio {
            mem: Mem::default(),
            policy: policy.clone(),
            cues: vec![],
            queries: vec![],
        };
        queue.advance(1.0 / 60.0, delta as i32, &policy, &mut native_audio);
        for (i, e) in queue.ring.entries.iter().enumerate() {
            let at = 0x7a6198 + i as u32 * 0x16c;
            assert_eq!(e.kind as u32, m.read32(at), "seed {seed} entry {i} kind");
            assert_eq!(
                e.age.to_bits(),
                m.read32(at + 0x144),
                "seed {seed} entry {i} age"
            );
            assert_eq!(
                queue.cues[i].unwrap_or(-1) as u32,
                m.read32(at + 0x15c),
                "seed {seed} entry {i} cue"
            );
            assert_eq!(
                queue.playing_handles[i].unwrap_or(-1) as u32,
                m.read32(at + 0x160),
                "seed {seed} entry {i} playback handle"
            );
            assert_eq!(e.text, vec![(b'a' + (i % 26) as u8), 0]);
        }
        assert_eq!(native_audio.cues, lifted_audio.cues, "seed {seed} cues");
        assert_eq!(
            native_audio.queries, lifted_audio.queries,
            "seed {seed} playback queries"
        );
    }
}

#[test]
fn native_switch_events_match_lifted_setters() {
    use tsfp_ports::c2r::t1925_weapons as original;
    for seed in 0..256u32 {
        for event in [
            WeaponPanelEvent::Selected,
            WeaponPanelEvent::NextInGroup,
            WeaponPanelEvent::PreviousInGroup,
            WeaponPanelEvent::Hide,
        ] {
            let mut s = WeaponAnimation {
                phase: seed % 4,
                flash: 0.25,
                alpha: 0.5,
                slide: 3.0,
                target: -4.0,
                cycle: (seed % 9) as f32 * 0.25 - 1.0,
            };
            let mut m = Mem::default();
            m.write32(0x7b0c7c, 0x10000);
            m.write32(0x10004, 0);
            m.write32(0x475c78, 1f32.to_bits());
            m.write32(0x4760d0, (-1f32).to_bits());
            m.write32(0x7a78b0, s.phase);
            for (off, v) in [
                (0x1754, s.flash),
                (0x1758, s.alpha),
                (0x175c, s.slide),
                (0x1760, s.target),
                (0x1764, s.cycle),
            ] {
                m.write32(0x7a6160 + off, v.to_bits());
            }
            let mut r = ExtRegisters {
                eax: 0,
                ecx: 0,
                edx: 0,
                esp: 0x9000,
                ebx: 0,
                ebp: 0,
                esi: 0,
                edi: 0,
                eflags: 0,
                fpu: Fpu {
                    stack: [0.0; 8],
                    top: 0,
                    control: 0,
                },
                xmm: [Xmm::default(); 5],
                xmm_hi: [Xmm::default(); 3],
            };
            match event {
                WeaponPanelEvent::Show => unreachable!("separate explicit-context contract test"),
                WeaponPanelEvent::Selected => tsfp_ports::c2r::t2049_w7_005::game_current_slot_7a6160_set_flag_0x1750_reset_0x1754_0x1764(&mut m,&mut r),
                WeaponPanelEvent::NextInGroup => original::game_current_slot_7a6160_step_field_0x1764_plus_one_set_state_1(&mut m,&mut r),
                WeaponPanelEvent::PreviousInGroup => original::game_current_slot_7a6160_step_field_0x1764_minus_one_set_state_1(&mut m,&mut r),
                WeaponPanelEvent::Hide => original::game_current_slot_7a6160_set_field_0x1750_3(&mut m,&mut r),
            }
            s.apply_event(event, 1.0);
            assert_eq!(s.phase, m.read32(0x7a78b0));
            for (off, v) in [
                (0x1754, s.flash),
                (0x1758, s.alpha),
                (0x175c, s.slide),
                (0x1760, s.target),
                (0x1764, s.cycle),
            ] {
                assert_eq!(
                    v.to_bits(),
                    m.read32(0x7a6160 + off),
                    "seed {seed}, event {event:?}, field {off:x}"
                );
            }
        }
    }
}

#[test]
fn queue_ages_and_retires_owned_payload_without_losing_text() {
    use tsfp_ui_model::messages::Message;
    let mut queue = OwnedMessageQueue::default();
    queue.push(
        Message {
            kind: 2,
            text: b"fixture".to_vec(),
            age: -0.1,
            end: 0.3,
            evict_after: 0.2,
            weight: 1,
            ..Default::default()
        },
        0.5,
        Some(7),
        None,
    );
    let policy = MessagePolicy {
        limits: [4; 14],
        conflicts: vec![],
    };
    let mut audio = Audio {
        mem: Mem::default(),
        policy: policy.clone(),
        cues: vec![],
        queries: vec![],
    };
    queue.advance(0.2, 1, &policy, &mut audio);
    assert_eq!(audio.cues, vec![7]);
    assert_eq!(queue.cues[0], None);
    assert_eq!(queue.ring.entries[0].kind, 2);
    assert_eq!(queue.ring.entries[0].text, b"fixture");
    queue.advance(0.2, 1, &policy, &mut audio);
    assert_eq!(queue.ring.entries[0].kind, 0);
    assert_eq!(audio.cues, vec![7]);
}

#[test]
fn hud_state_advances_only_the_valid_owned_player_queue() {
    use tsfp_ui_model::{hud_native::HudState, messages::Message};

    let policy = MessagePolicy {
        limits: [i32::MAX; 14],
        conflicts: vec![],
    };
    let mut hud = HudState::default();
    hud.messages[1].push(
        Message {
            kind: 2,
            text: b"native expiry".to_vec(),
            age: -0.1,
            end: 0.05,
            evict_after: 0.05,
            weight: 1,
            ..Default::default()
        },
        0.0,
        Some(19),
        None,
    );
    let original = hud.messages[1].clone();
    let mut audio = Audio {
        mem: Mem::default(),
        policy: policy.clone(),
        cues: vec![],
        queries: vec![],
    };

    assert!(!hud.advance_messages(-1, 0.2, 1, &policy, &mut audio));
    assert!(!hud.advance_messages(4, 0.2, 1, &policy, &mut audio));
    assert!(hud.advance_messages(1, 0.2, 0, &policy, &mut audio));
    assert_eq!(hud.messages[1], original);
    assert!(audio.cues.is_empty());

    assert!(hud.advance_messages(1, 0.2, 1, &policy, &mut audio));
    assert_eq!(hud.messages[1].ring.entries[0].kind, 0);
    assert_eq!(hud.messages[1].ring.entries[0].text, b"native expiry");
    assert_eq!(hud.messages[1].cues[0], None);
    assert_eq!(audio.cues, vec![19]);
    assert!(
        hud.messages[0]
            .ring
            .entries
            .iter()
            .all(|entry| entry.kind == 0)
    );
}

#[test]
fn hud_state_publishes_a_message_cue_only_for_successful_native_admission() {
    use tsfp_ui_model::{
        hud_native::HudState,
        hud_producers::{GenericMessagePreset, MessageAdmission},
    };

    let mut hud = HudState::default();
    let preset = GenericMessagePreset::default();
    let MessageAdmission::Inserted(id) =
        hud.insert_message_with_cue(0, 2, b"owned cue", 0.0, 31, &preset)
    else {
        panic!("valid owned message should be admitted");
    };
    assert_eq!(hud.messages[0].cues[id.slot()], Some(31));

    assert_eq!(
        hud.insert_message_with_cue(0, 2, b"owned cue", 0.0, 99, &preset),
        MessageAdmission::Duplicate
    );
    assert_eq!(hud.messages[0].cues[id.slot()], Some(31));
    assert_eq!(
        hud.insert_message_with_cue(-1, 2, b"invalid", 0.0, 99, &preset),
        MessageAdmission::Invalid
    );
}

#[test]
fn published_game_facts_preserve_owned_hud_lifecycles() {
    use tsfp_ui_model::hud_native::{HudFrame, HudState};
    let mut hud = HudState::default();
    assert!(hud.weapon_event(1, WeaponPanelEvent::NextInGroup, 1.0));
    hud.weapons[1].set_slide_target(-8.0);
    assert!(hud.publish_frame(HudFrame {
        slot: 1,
        health: 25.0,
        ..Default::default()
    }));
    assert!(hud.publish_frame(HudFrame {
        slot: 1,
        health: 24.0,
        ..Default::default()
    }));
    assert_eq!(hud.weapons[1].phase, 1);
    assert_eq!(hud.weapons[1].target, -8.0);
    assert_eq!(hud.weapons[1].cycle, 1.0);
    assert_eq!(hud.frames[1].as_ref().unwrap().health, 24.0);
    assert!(!hud.weapon_event(4, WeaponPanelEvent::Hide, 1.0));
    assert!(!hud.publish_frame(HudFrame {
        slot: u32::MAX,
        ..Default::default()
    }));
    assert!(hud.reset_slot(1));
    assert_eq!(hud, HudState::default());
}

/// Tier 0, FABRICATED event sequence: the retained field contract is important
/// when an entry previously held a sound-bearing message, then wraps to a notice.
#[test]
fn native_notice_wrap_preserves_unwritten_metadata() {
    use tsfp_ui_model::{
        hud_native::HudState,
        messages::{Message, MessageTuning},
    };
    let mut hud = HudState::default();
    let tuning = MessageTuning {
        fade_start: 2.0,
        end: 3.0,
        one: 1.0,
    };
    hud.messages[2].push(
        Message {
            kind: 2,
            ..Default::default()
        },
        0.25,
        Some(7),
        Some(8),
    );
    for i in 1..16 {
        assert!(hud.message_notice(2, NoticeKind::Plain, b"fixture", i, 0.0, &tuning));
    }
    assert!(hud.message_notice(2, NoticeKind::VariantC, b"new\0discard", 99, 0.5, &tuning));
    let queue = &hud.messages[2];
    assert_eq!(queue.ring.cursor, 1);
    assert_eq!(queue.ring.entries[0].kind, 12);
    assert_eq!(queue.ring.entries[0].text, b"new");
    assert_eq!(queue.ring.entries[0].age, -0.5);
    assert_eq!(queue.ring.entries[0].opaque, 99);
    assert_eq!(queue.slides[0], 0.25);
    assert_eq!(queue.cues[0], Some(7));
    assert_eq!(queue.playing_handles[0], Some(8));
    let before = hud.clone();
    assert!(!hud.message_notice(-1, NoticeKind::Plain, b"ignored", 0, 0.0, &tuning));
    assert!(!hud.message_notice(4, NoticeKind::Plain, b"ignored", 0, 0.0, &tuning));
    assert!(!hud.clear_messages(-1, -1));
    assert!(!hud.clear_messages(4, -1));
    assert_eq!(hud, before);
}

#[test]
fn native_clear_preserves_retained_title_and_sound_fields() {
    use tsfp_ui_model::{hud_native::HudState, messages::Message};
    let mut hud = HudState::default();
    for kind in [2, 4, 5, 13, 10] {
        hud.messages[1].push(
            Message {
                kind,
                text: b"fixture".to_vec(),
                age: 0.5,
                ..Default::default()
            },
            0.25,
            Some(7),
            Some(8),
        );
    }
    let before = hud.messages[1].clone();
    assert!(hud.clear_messages(1, -1));
    for i in 0..5 {
        let mut expected = before.ring.entries[i].clone();
        if ![4, 5, 13].contains(&expected.kind) {
            expected.kind = 0;
        }
        assert_eq!(hud.messages[1].ring.entries[i], expected);
    }
    assert_eq!(hud.messages[1].slides, before.slides);
    assert_eq!(hud.messages[1].cues, before.cues);
    assert_eq!(hud.messages[1].playing_handles, before.playing_handles);
    assert_eq!(hud.messages[1].ring.cursor, before.ring.cursor);
    assert!(hud.clear_messages(1, 4));
    assert_eq!(hud.messages[1].ring.entries[1].kind, 0);
    assert_eq!(hud.messages[1].ring.entries[2].kind, 5);
}

/// Tier 0 FABRICATED event contract; newest same-kind only, full source comparison.
#[test]
fn generic_insertion_deduplicates_only_newest_same_kind() {
    use tsfp_ui_model::hud_producers::{GenericMessagePreset, MessageInsertion::*};
    let mut queue = OwnedMessageQueue::default();
    let preset = GenericMessagePreset::default();
    assert_eq!(queue.insert_generic(2, b"first", 0.0, &preset), Inserted(0));
    assert_eq!(queue.insert_generic(3, b"other", 0.0, &preset), Inserted(1));
    let before = queue.clone();
    assert_eq!(
        queue.insert_generic(2, b"first\0ignored", 0.0, &preset),
        Duplicate
    );
    assert_eq!(queue, before);
    assert_eq!(
        queue.insert_generic(2, b"second", 0.0, &preset),
        Inserted(2)
    );
    assert_eq!(queue.insert_generic(2, b"first", 0.0, &preset), Inserted(3));
    assert_eq!(
        queue.insert_generic(6, b"repeat", 0.0, &preset),
        Inserted(4)
    );
    assert_eq!(
        queue.insert_generic(6, b"repeat", 0.0, &preset),
        Inserted(5)
    );
    let long = vec![b'x'; 340];
    assert_eq!(queue.insert_generic(4, &long, 0.0, &preset), Inserted(6));
    assert_eq!(queue.ring.entries[6].text.len(), 319);
    assert_eq!(queue.insert_generic(4, &long, 0.0, &preset), Inserted(7));
    let before = queue.clone();
    for kind in [0, 14, u32::MAX] {
        assert_eq!(
            queue.insert_generic(kind, b"invalid", 0.0, &preset),
            Invalid
        );
    }
    assert_eq!(queue, before);
}

#[derive(Clone)]
struct MessageOracle {
    memory: Mem,
}

impl GuestCall for MessageOracle {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        match address {
            0x3c8960 => {
                let destination = self.memory.read32(registers.esp);
                let source = self.memory.read32(registers.esp + 4);
                let count = self.memory.read32(registers.esp + 8);
                let mut terminated = false;
                for offset in 0..count {
                    let byte = if terminated {
                        0
                    } else {
                        let byte = self.memory.read8(source + offset);
                        terminated = byte == 0;
                        byte
                    };
                    self.memory.write8(destination + offset, byte);
                }
                registers.eax = destination;
            }
            0x12d350 => {
                let caller_esp = registers.esp;
                registers.esp = registers.esp.wrapping_sub(4);
                self.memory.write32(registers.esp, 0xdead_beef);
                let mut memory = self.memory.clone();
                tsfp_ports::c2r::ui_hud::game_hud_player_message_post_by_kind_1_to_0xd(
                    &mut memory,
                    self,
                    registers,
                );
                registers.esp = caller_esp;
                if registers.eax != 0 {
                    let copied_text = registers.eax + 4;
                    for offset in 0..tsfp_ui_model::messages::TEXT_BYTES as u32 {
                        memory.write8(
                            copied_text + offset,
                            self.memory.read8(copied_text + offset),
                        );
                    }
                }
                self.memory = memory;
            }
            _ => panic!("unexpected message oracle call {address:#x}"),
        }
    }
}

fn message_registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

/// Tier 1, FABRICATED finite inputs versus lifted 0x12D350/0x148A10. The
/// runtime switch is fixed to its documented no-extra-writes branch so this
/// checks queue admission/copy/identity/cue publication, not preset decoding.
#[test]
fn owned_message_admission_and_cue_dispatch_match_lifted_originals() {
    use tsfp_ui_model::hud_producers::{GenericMessagePreset, MessageAdmission};

    let mut seed = 0x73a5_20d1u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for sequence in 0..128 {
        let mut queue = OwnedMessageQueue::default();
        let mut memory = Mem::default();
        for kind in 1..=13 {
            memory.write32(0x12d678 + (kind - 1) * 4, 0x12d66e);
        }
        for index in 0..16 {
            let entry = 0x7a6198 + index * 0x16c;
            memory.write32(entry + 0x15c, u32::MAX);
            memory.write32(entry + 0x160, u32::MAX);
        }
        let mut oracle = MessageOracle {
            memory: memory.clone(),
        };

        for step in 0..32 {
            let value = next();
            let kind = (value % 13) + 1;
            let text = match value % 7 {
                0 => b"same message".to_vec(),
                1 => b"same message\0ignored".to_vec(),
                2 => b"changed message".to_vec(),
                3 => (0..(300 + (value as usize % 90)))
                    .map(|index| b'a' + ((index + step as usize) % 23) as u8)
                    .collect(),
                _ => format!("message {sequence}:{step}:{}", value % 5).into_bytes(),
            };
            let delay: f32 = [0.0, -0.0, 0.25, 1.0][(value as usize >> 8) % 4];
            let cue = if step % 2 == 0 {
                Some((value >> 16) as i32)
            } else {
                None
            };
            let source = 0x20_000;
            for (offset, byte) in text.iter().copied().enumerate() {
                memory.write8(source + offset as u32, byte);
            }
            memory.write8(source + text.len() as u32, 0);
            let esp = 0x9000;
            memory.write32(esp, 0xdead_beef);
            memory.write32(esp + 4, 0);
            memory.write32(esp + 8, kind);
            memory.write32(esp + 12, source);
            memory.write32(esp + 16, delay.to_bits());
            if let Some(cue) = cue {
                memory.write32(esp + 20, cue as u32);
            }
            let mut registers = message_registers(esp);
            oracle.memory = memory.clone();

            let admitted = match cue {
                Some(cue) => queue.admit_generic_with_cue(
                    kind,
                    &text,
                    delay,
                    cue,
                    &GenericMessagePreset::default(),
                ),
                None => queue.admit_generic(kind, &text, delay, &GenericMessagePreset::default()),
            };
            if cue.is_some() {
                tsfp_ports::c2r::ui_hud_2::game_call_12d350_then_set_result_field_0x15c(
                    &mut memory,
                    &mut oracle,
                    &mut registers,
                );
            } else {
                tsfp_ports::c2r::ui_hud::game_hud_player_message_post_by_kind_1_to_0xd(
                    &mut memory,
                    &mut oracle,
                    &mut registers,
                );
            }
            if cue.is_some() {
                if registers.eax != 0 {
                    oracle
                        .memory
                        .write32(registers.eax + 0x15c, memory.read32(registers.eax + 0x15c));
                }
                memory = oracle.memory.clone();
            } else {
                if registers.eax != 0 {
                    for offset in 0..tsfp_ui_model::messages::TEXT_BYTES as u32 {
                        memory.write8(
                            registers.eax + 4 + offset,
                            oracle.memory.read8(registers.eax + 4 + offset),
                        );
                    }
                }
                oracle.memory = memory.clone();
            }
            let expected_return = match admitted {
                MessageAdmission::Inserted(id) => 0x7a6160 + 0x38 + id.slot() as u32 * 0x16c,
                MessageAdmission::Invalid | MessageAdmission::Duplicate => 0,
            };
            assert_eq!(
                registers.eax, expected_return,
                "seed {sequence}, step {step}"
            );
            assert_eq!(memory.read32(0x7a6190), queue.ring.cursor as u32);

            for (index, entry) in queue.ring.entries.iter().enumerate() {
                let original = 0x7a6198 + index as u32 * 0x16c;
                assert_eq!(
                    memory.read32(original),
                    entry.kind,
                    "kind {sequence}/{step}/{index}"
                );
                assert_eq!(
                    memory.read32(original + 0x144),
                    entry.age.to_bits(),
                    "age {sequence}/{step}/{index}"
                );
                assert_eq!(
                    memory.read32(original + 0x148),
                    entry.fade_start.to_bits(),
                    "fade {sequence}/{step}/{index}"
                );
                assert_eq!(
                    memory.read32(original + 0x150),
                    entry.end.to_bits(),
                    "end {sequence}/{step}/{index}"
                );
                assert_eq!(
                    memory.read32(original + 0x154),
                    entry.evict_after.to_bits(),
                    "eviction {sequence}/{step}/{index}"
                );
                assert_eq!(memory.read32(original + 0x158), entry.weight);
                assert_eq!(
                    memory.read32(original + 0x15c),
                    queue.cues[index].unwrap_or(-1) as u32
                );
                assert_eq!(
                    memory.read32(original + 0x160),
                    queue.playing_handles[index].unwrap_or(-1) as u32
                );
                assert_eq!(memory.read32(original + 0x164), queue.progress[index]);
                assert_eq!(memory.read32(original + 0x168), entry.opaque);
                let expected = entry
                    .text
                    .get(..tsfp_ui_model::messages::TEXT_BYTES - 1)
                    .unwrap_or(&entry.text);
                for offset in 0..expected.len() {
                    assert_eq!(
                        memory.read8(original + 4 + offset as u32),
                        expected[offset],
                        "text {sequence}/{step}/{index}/{offset}"
                    );
                }
                assert_eq!(
                    memory.read8(original + 0x143),
                    0,
                    "terminator {sequence}/{step}/{index}"
                );
            }
        }
    }
}

#[test]
fn message_ids_expire_on_reuse_clear_and_queue_reset() {
    use tsfp_ui_model::hud_producers::{GenericMessagePreset, MessageAdmission};
    let mut queue = OwnedMessageQueue::default();
    let preset = GenericMessagePreset::default();
    let MessageAdmission::Inserted(first) = queue.admit_generic(2, b"first", 0.0, &preset) else {
        panic!("first message should be admitted")
    };
    assert_eq!(queue.message(first).unwrap().text, b"first");
    for kind in 1..=13 {
        assert!(matches!(
            queue.admit_generic(kind, format!("kind {kind}").as_bytes(), 0.0, &preset),
            MessageAdmission::Inserted(_)
        ));
    }
    assert!(matches!(
        queue.admit_generic(1, b"tail", 0.0, &preset),
        MessageAdmission::Inserted(_)
    ));
    let MessageAdmission::Inserted(last) = queue.admit_generic(2, b"next", 0.0, &preset) else {
        panic!("next same-kind message should be admitted")
    };
    let MessageAdmission::Inserted(reused) = queue.admit_generic(2, b"reused", 0.0, &preset) else {
        panic!("ring reuse should be admitted")
    };
    assert_eq!(reused.slot(), first.slot());
    assert!(reused.generation() > first.generation());
    assert!(queue.message(first).is_none());
    assert_eq!(queue.message(last).unwrap().text, b"next");
    queue.clear_kind(2);
    assert!(queue.message(last).is_none());
    queue.reset();
    let MessageAdmission::Inserted(after_reset) =
        queue.admit_generic(2, b"after reset", 0.0, &preset)
    else {
        panic!("post-reset message should be admitted")
    };
    assert!(after_reset.generation() > reused.generation());
    assert!(queue.message(reused).is_none());

    struct SilentAudio;
    impl HudSounds for SilentAudio {
        fn is_playing(&mut self, _handle: i32) -> bool {
            false
        }
        fn submit(&mut self, cue: i32) {
            panic!("unexpected cue submission {cue}")
        }
    }
    queue.advance(
        1.0,
        1,
        &MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: Vec::new(),
        },
        &mut SilentAudio,
    );
    assert!(queue.message(after_reset).is_none());
}

#[test]
fn generic_wrap_resets_only_written_fields_and_player_progress() {
    use tsfp_ui_model::{
        hud_native::HudState,
        hud_producers::{GenericMessagePreset, MessageInsertion::*},
        messages::Message,
    };
    let mut hud = HudState::default();
    hud.messages[1].push(
        Message {
            kind: 2,
            fade_start: 7.0,
            end: 8.0,
            evict_after: 9.0,
            ..Default::default()
        },
        0.25,
        Some(7),
        Some(8),
    );
    hud.messages[1].progress[0] = 99;
    hud.messages[1].ring.cursor = 0;
    hud.input_progress = [1, 2, 3, 4];
    let preset = GenericMessagePreset {
        reset_input_progress: true,
        ..Default::default()
    };
    assert_eq!(
        hud.insert_message(1, 3, b"fixture", 0.5, &preset),
        Inserted(0)
    );
    let queue = &hud.messages[1];
    assert_eq!(queue.ring.entries[0].age, -0.5);
    assert_eq!(queue.ring.entries[0].opaque, u32::MAX);
    assert_eq!(queue.ring.entries[0].weight, 1);
    assert_eq!(queue.ring.entries[0].fade_start, 7.0);
    assert_eq!(queue.ring.entries[0].end, 8.0);
    assert_eq!(queue.ring.entries[0].evict_after, 9.0);
    assert_eq!(queue.slides[0], 0.25);
    assert_eq!(queue.cues[0], None);
    assert_eq!(queue.playing_handles[0], None);
    assert_eq!(queue.progress[0], 0);
    assert_eq!(hud.input_progress, [1, 0, 3, 4]);
    hud.input_progress[1] = 42;
    let before = hud.clone();
    assert_eq!(
        hud.insert_message(1, 3, b"fixture", 0.0, &preset),
        Duplicate
    );
    for player in [-1, 4, i32::MAX] {
        assert_eq!(
            hud.insert_message(player, 3, b"invalid", 0.0, &preset),
            Invalid
        );
    }
    assert_eq!(hud, before);
    let preset = GenericMessagePreset {
        fade_start: Some(1.0),
        end: Some(2.0),
        evict_after: Some(3.0),
        slide: Some(4.0),
        weight: Some(5),
        reset_input_progress: false,
    };
    assert_eq!(hud.insert_message(1, 5, b"next", 0.0, &preset), Inserted(1));
    assert_eq!(hud.messages[1].ring.entries[1].fade_start, 1.0);
    assert_eq!(hud.messages[1].ring.entries[1].end, 2.0);
    assert_eq!(hud.messages[1].ring.entries[1].evict_after, 3.0);
    assert_eq!(hud.messages[1].ring.entries[1].weight, 5);
    assert_eq!(hud.messages[1].slides[1], 4.0);
    assert_eq!(hud.input_progress[1], 42);
    assert!(hud.reset_slot(1));
    assert_eq!(hud.input_progress[1], 0);
}

#[test]
fn runtime_message_presets_reject_unknown_and_preserve_branch_contracts() {
    struct Data(u32);
    impl MessagePresetData for Data {
        fn word(&self, address: u32) -> Option<u32> {
            if address == 0x12d678 {
                Some(self.0)
            } else {
                Some(2.0f32.to_bits())
            }
        }
    }
    struct Metrics;
    impl MessageTextMetrics for Metrics {
        fn outlined_weight(&self, text: &[u8]) -> Option<i32> {
            Some(text.len() as i32)
        }
        fn text_count(&self, text: &[u8]) -> Option<i32> {
            Some(text.len() as i32)
        }
    }
    let load =
        |branch, text: &[u8]| GenericMessagePreset::load_original(1, text, &Data(branch), &Metrics);
    assert_eq!(
        load(0xdeadbeef, b"x"),
        Err(MessagePresetError::UnknownBranch(0xdeadbeef))
    );
    assert_eq!(
        load(0x12d66e, b"x").unwrap(),
        GenericMessagePreset::default()
    );
    assert_eq!(
        GenericMessagePreset::load_original(0, b"", &Data(0), &Metrics),
        Err(MessagePresetError::InvalidKind)
    );
    let counted = load(0x12d522, b"abc\0ignored").unwrap();
    assert_eq!(counted.fade_start, Some(8.0));
    assert_eq!(counted.end, Some(10.0));
    assert!(counted.reset_input_progress);
    assert_eq!(load(0x12d4d5, &vec![b'x'; 400]).unwrap().weight, Some(319));
    assert_eq!(load(0x12d585, &vec![b'x'; 400]).unwrap().end, Some(762.0));
    assert_eq!(load(0x12d585, b"short").unwrap().end, Some(2.0));
    for branch in [0x12d4a6, 0x12d570, 0x12d5e2, 0x12d60c, 0x12d636] {
        let preset = load(branch, b"text").unwrap();
        assert_eq!(preset.fade_start, Some(2.0));
        assert_eq!(preset.end, Some(2.0));
        assert_eq!(preset.evict_after, Some(2.0));
        assert_eq!(
            preset.slide.is_some(),
            matches!(branch, 0x12d5e2 | 0x12d60c | 0x12d636)
        );
    }
    struct Missing;
    impl MessagePresetData for Missing {
        fn word(&self, _: u32) -> Option<u32> {
            None
        }
    }
    assert_eq!(
        GenericMessagePreset::load_original(1, b"", &Missing, &Metrics),
        Err(MessagePresetError::MissingData)
    );
}

#[test]
fn progress_consumer_draws_before_retiring_and_shares_player_progress() {
    let mut queue = OwnedMessageQueue::default();
    queue.ring.entries[0].kind = 7;
    queue.ring.entries[0].text = b"native notice".to_vec();
    queue.ring.entries[1].kind = 7;
    queue.cues[0] = Some(42);
    queue.progress[0] = 99;
    let mut progress = 0;
    for expected in 1..=3 {
        let index = if expected == 2 { 1 } else { 0 };
        let result = queue.consume_progress_notice(index, &mut progress, true, |message, value| {
            (message.kind, value, message.text.clone())
        });
        assert_eq!(result.as_ref().map(|r| (r.0, r.1)), Some((7, expected)));
    }
    assert_eq!(progress, 0);
    assert_eq!(queue.ring.entries[0].kind, 0);
    assert_eq!(queue.ring.entries[1].kind, 7);
    assert_eq!(queue.cues[0], Some(42));
    assert_eq!(queue.progress[0], 99);
    assert_eq!(queue.ring.entries[0].text, b"native notice");
    assert_eq!(
        queue.consume_progress_notice(0, &mut progress, true, |_, _| panic!("unused")),
        None::<()>
    );
    assert_eq!(
        queue.consume_progress_notice(16, &mut progress, true, |_, _| panic!("invalid")),
        None::<()>
    );
    assert_eq!(progress, 0);
    progress = u32::MAX;
    assert_eq!(
        queue.consume_progress_notice(1, &mut progress, true, |_, p| p),
        Some(0)
    );
    progress = 3;
    assert_eq!(
        queue.consume_progress_notice(1, &mut progress, false, |m, p| (m.kind, p)),
        Some((7, 3))
    );
    assert_eq!(progress, 0);
    assert_eq!(queue.ring.entries[1].kind, 0);
}

#[test]
fn native_scheduler_filters_before_actions_and_counts_before_retirement() {
    let mut queue = OwnedMessageQueue::default();
    queue.ring.cursor = 14;
    for (index, kind, age, weight) in [
        (14, 2, 1.0, 2),
        (15, 2, 9.5, 2),
        (0, 7, 0.0, 1),
        (1, 7, 0.0, 1),
        (2, 7, -1.0, 1),
        (3, 7, f32::NAN, 1),
        (4, 8, 0.0, 3),
        (5, 9, 0.0, 1),
        (6, 1, 0.0, 1),
        (7, 6, 0.0, 1),
        (8, 11, 0.0, 1),
    ] {
        let entry = &mut queue.ring.entries[index];
        entry.kind = kind;
        entry.age = age;
        entry.weight = weight;
        entry.end = 10.0;
    }
    let mut policy = MessagePolicy {
        limits: [10; 14],
        conflicts: vec![(9, 1)],
    };
    policy.limits[7] = 1;
    let mut progress = 2;
    let mut actions = 0;
    let mut draws = Vec::new();
    let sums = queue.replay_native(
        &policy,
        false,
        false,
        &mut progress,
        || {
            actions += 1;
            true
        },
        |m, _, offset, p| draws.push((m.kind, offset, p)),
    );
    assert_eq!(
        draws,
        vec![(2, 0.0, 2), (2, 2.0, 2), (7, 3.0, 3), (8, 3.0, 0)]
    );
    assert_eq!(actions, 1);
    assert_eq!(progress, 0);
    assert_eq!(queue.ring.entries[0].kind, 0);
    assert_eq!(queue.ring.entries[1].kind, 7);
    assert_eq!(sums[7], 1); // retired notice still hides the next one this call
    assert_eq!(sums[8], 0);
    assert_eq!(sums[9], 1); // hidden split presentation still participates
    let before = queue.clone();
    draws.clear();
    let sums = queue.replay_native(
        &policy,
        true,
        true,
        &mut progress,
        || panic!("only-kind filtering must precede native input"),
        |m, _, _, _| draws.push((m.kind, 0.0, 0)),
    );
    assert_eq!(draws, vec![(9, 0.0, 0)]);
    assert_eq!(sums[9], 1);
    assert_eq!(
        queue.ring.entries[3].age.to_bits(),
        before.ring.entries[3].age.to_bits()
    );
    let mut before = before;
    before.ring.entries[3].age = 0.0;
    queue.ring.entries[3].age = 0.0;
    assert_eq!(queue, before);
}

#[test]
fn corner_notice_uses_owned_payload_native_metrics_and_runtime_layout() {
    use tsfp_ui_model::messages::Message;
    struct Text;
    impl CornerNoticeText for Text {
        fn fit_corner_font(&self, text: &[u8]) -> Option<u32> {
            assert_eq!(text, b"notice");
            Some(42)
        }
        fn alpha_integer(&self, value: f64) -> i32 {
            value as i32
        }
    }
    let layout = CornerNoticeLayout {
        viewport_left: 100,
        margin_left: 7,
        viewport_bottom: 480,
        line_height: 20.0,
        height_factor: 2.0,
        baseline_offset: 3.5,
        full_alpha: 1.0,
        alpha_scale: 120.0,
        colour_base: 0x12345600,
    };
    let mut message = Message {
        kind: 8,
        text: b"notice\0ignored".to_vec(),
        age: 3.0,
        fade_start: 2.0,
        end: 4.0,
        ..Default::default()
    };
    let before = message.clone();
    let draw = corner_notice(&message, &layout, &Text).unwrap();
    assert_eq!(draw.position, [107, 443]);
    assert_eq!(draw.font, 42);
    assert_eq!(draw.text, b"notice");
    assert_eq!(draw.colour, 0x1234563c);
    assert_eq!(corner_notice(&message, &layout, &Text), Some(draw));
    assert_eq!(message, before);
    message.age = f32::NAN;
    assert_eq!(
        corner_notice(&message, &layout, &Text).unwrap().colour,
        0x12345678
    );
    let invalid = CornerNoticeLayout {
        viewport_left: i32::MAX,
        margin_left: 1,
        baseline_offset: f32::NAN,
        ..layout
    };
    assert_eq!(
        corner_notice(&message, &invalid, &Text).unwrap().position,
        [i32::MIN, i32::MIN]
    );
    struct Missing;
    impl CornerNoticeText for Missing {
        fn fit_corner_font(&self, _: &[u8]) -> Option<u32> {
            None
        }
        fn alpha_integer(&self, _: f64) -> i32 {
            panic!("missing font must reject draw")
        }
    }
    assert!(corner_notice(&message, &layout, &Missing).is_none());
    message.kind = 1;
    assert!(corner_notice(&message, &layout, &Text).is_none());
}

#[test]
fn pickup_notice_uses_owned_payload_stack_fade_and_typed_layout() {
    use std::cell::RefCell;
    use tsfp_ui_model::messages::Message;

    struct Text(RefCell<Vec<&'static str>>);
    impl PickupNoticeText for Text {
        fn fit_pickup_font(&self, bytes: &[u8]) -> Option<u32> {
            self.0.borrow_mut().push("fit");
            assert_eq!(bytes, b"Picked up");
            Some(17)
        }
        fn line_top(&self, font: u32) -> Option<f32> {
            self.0.borrow_mut().push("line_top");
            assert_eq!(font, 17);
            Some(20.0)
        }
        fn width(&self, font: u32, bytes: &[u8]) -> Option<f32> {
            self.0.borrow_mut().push("width");
            assert_eq!(font, 17);
            assert_eq!(bytes, b"Picked up");
            Some(100.0)
        }
        fn integer(&self, value: f32) -> i32 {
            value as i32
        }
    }

    let message = Message {
        kind: 2,
        text: b"Picked up\0unused".to_vec(),
        age: 3.0,
        fade_start: 2.0,
        end: 4.0,
        ..Default::default()
    };
    let before = message.clone();
    let layout = PickupNoticeLayout {
        center_x: 400,
        bottom_line: 400,
        max_width: 200.0,
        hide_stacked: true,
        alpha_scale: 127.0,
        foreground_base: 0x6464_6400,
        outline_base: 0x1e1e_1e00,
    };
    let text = Text(RefCell::new(Vec::new()));
    let draw = pickup_notice(&message, 0.25, &layout, &text).unwrap();
    assert_eq!(draw.position, [350, 405]);
    assert_eq!(draw.text, b"Picked up");
    assert_eq!(draw.font, 17);
    assert_eq!(draw.foreground, 0x6464_642f);
    assert_eq!(draw.outline, 0x1e1e_1e2f);
    assert!(draw.visible);
    assert_eq!(*text.0.borrow(), ["fit", "line_top", "width"]);
    assert_eq!(message, before);

    let hidden = PickupNoticeLayout {
        max_width: 100.0,
        hide_stacked: false,
        ..layout
    };
    let draw = pickup_notice(&message, 2.0, &hidden, &text).unwrap();
    assert_eq!(draw.foreground, 0x6464_643f);
    assert!(!draw.visible);
    assert!(pickup_notice(&Message { kind: 1, ..message }, 0.0, &layout, &text).is_none());
}

#[test]
fn progress_notice_reveals_before_native_fit_and_wrap_without_mutation() {
    use std::cell::RefCell;
    use tsfp_ui_model::messages::Message;
    struct Text {
        calls: RefCell<Vec<&'static str>>,
        bad_offset: bool,
    }
    impl ProgressNoticeText for Text {
        fn count(&self, _: &[u8]) -> Option<u32> {
            self.calls.borrow_mut().push("count");
            Some(4)
        }
        fn prefix_offset(&self, _: &[u8], count: u32) -> Option<usize> {
            self.calls.borrow_mut().push("prefix");
            Some(if self.bad_offset { 999 } else { count as usize })
        }
        fn fit_font(&self, _: &[u8]) -> Option<u32> {
            self.calls.borrow_mut().push("font");
            Some(9)
        }
        fn wrap(&self, text: &[u8], width: f32) -> Option<Vec<u8>> {
            self.calls.borrow_mut().push("wrap");
            assert_eq!(width, 580.0);
            Some(text.to_vec())
        }
        fn alpha_integer(&self, value: f64) -> i32 {
            self.calls.borrow_mut().push("alpha");
            value as i32
        }
    }
    let text = Text {
        calls: RefCell::new(Vec::new()),
        bad_offset: false,
    };
    let layout = ProgressNoticeLayout {
        viewport_left: 100,
        viewport_right: 740,
        viewport_bottom: 480,
        margin_left: 7,
        margin_right: 13,
        margin_bottom: 3,
        effective_players: 1,
        scale: [0.5, 0.5],
        reveal_rate: 2.0,
        full_alpha: 1.0,
        alpha_scale: 120.0,
        foreground_base: 0xabcdef00,
        outline_base: 0x12345600,
    };
    let mut message = Message {
        kind: 7,
        text: b"abcdef\0ignored".to_vec(),
        age: 1.5,
        fade_start: 1.0,
        end: 2.0,
        ..Default::default()
    };
    let before = message.clone();
    let draw = progress_notice(&message, 1, &layout, &text).unwrap();
    assert_eq!(draw.text, b"abc");
    assert_eq!(draw.font, 9);
    assert_eq!(draw.position, [63.0, 523.0]);
    assert_eq!(draw.foreground, 0xabcdef3c);
    assert_eq!(draw.outline, 0x1234563c);
    assert_eq!(*text.calls.borrow(), ["prefix", "alpha", "font", "wrap"]);
    assert_eq!(message, before);
    text.calls.borrow_mut().clear();
    let draw = progress_notice(&message, 2, &layout, &text).unwrap();
    assert_eq!(draw.text, b"abcd");
    assert_eq!(
        *text.calls.borrow(),
        ["count", "prefix", "alpha", "font", "wrap"]
    );
    message.age = -1.0;
    assert_eq!(
        progress_notice(&message, 0, &layout, &text).unwrap().text,
        b"abcdef"
    );
    message.age = f32::NAN;
    let split = ProgressNoticeLayout {
        effective_players: 2,
        ..layout
    };
    let draw = progress_notice(&message, 3, &split, &text).unwrap();
    assert_eq!(draw.text, b"abcdef");
    assert_eq!(draw.position[1], 503.0);
    assert_eq!(draw.foreground, 0xabcdef78);
    let bad = Text {
        calls: RefCell::new(Vec::new()),
        bad_offset: true,
    };
    assert!(progress_notice(&message, 0, &layout, &bad).is_none());
    assert_eq!(*bad.calls.borrow(), ["prefix"]);
    message.kind = 8;
    assert!(progress_notice(&message, 0, &layout, &bad).is_none());
}

#[test]
fn split_notice_preserves_wrap_expression_font_gate_and_centered_layout() {
    use std::cell::RefCell;
    use tsfp_ui_model::messages::Message;
    struct Text {
        calls: RefCell<Vec<&'static str>>,
    }
    impl SplitNoticeText for Text {
        fn wrap(&self, font: u32, payload: &[u8], width: f32) -> Option<Vec<u8>> {
            self.calls.borrow_mut().push("wrap");
            assert!(font == 12 || font == 21);
            assert_eq!(payload, b"notice");
            assert_eq!(width, 357.0);
            Some(b"two\nrows".to_vec())
        }
        fn bottom_line(&self) -> Option<i32> {
            self.calls.borrow_mut().push("bottom");
            Some(400)
        }
    }
    let text = Text {
        calls: RefCell::new(Vec::new()),
    };
    let layout = SplitNoticeLayout {
        viewport_left: 100,
        viewport_right: 620,
        margin_left: 7,
        margin_right: 13,
        mode: 10,
        level_players: 2,
        alternate_context: true,
        normal_font: 21,
        alternate_font: 12,
        width_factor: 0.5,
        foreground: 0x12345678,
        outline: 0x87654321,
    };
    let message = Message {
        kind: 9,
        text: b"notice\0ignored".to_vec(),
        ..Default::default()
    };
    let before = message.clone();
    let draw = split_notice(&message, &layout, &text).unwrap();
    assert_eq!(draw.position, [357.0, 400.0]);
    assert_eq!(draw.font, 12);
    assert!(draw.centered);
    assert_eq!(draw.foreground, layout.foreground);
    assert_eq!(draw.outline, layout.outline);
    assert_eq!(*text.calls.borrow(), ["wrap", "bottom"]);
    assert_eq!(message, before);
    for changed in [
        SplitNoticeLayout { mode: 9, ..layout },
        SplitNoticeLayout {
            level_players: 1,
            ..layout
        },
        SplitNoticeLayout {
            alternate_context: false,
            ..layout
        },
    ] {
        assert_eq!(split_notice(&message, &changed, &text).unwrap().font, 21);
    }
    struct Missing;
    impl SplitNoticeText for Missing {
        fn wrap(&self, _: u32, _: &[u8], _: f32) -> Option<Vec<u8>> {
            None
        }
        fn bottom_line(&self) -> Option<i32> {
            panic!("wrap rejection must stop")
        }
    }
    assert!(split_notice(&message, &layout, &Missing).is_none());
    assert!(split_notice(&Message { kind: 1, ..message }, &layout, &Missing).is_none());
}

#[test]
fn center_notice_uses_fitted_width_and_explicit_left_alignment() {
    use std::cell::RefCell;
    use tsfp_ui_model::messages::Message;
    struct Text {
        calls: RefCell<Vec<&'static str>>,
        missing: bool,
    }
    impl CenterNoticeText for Text {
        fn fit_font(&self, payload: &[u8]) -> Option<u32> {
            self.calls.borrow_mut().push("font");
            assert_eq!(payload, b"a\nb");
            (!self.missing).then_some(17)
        }
        fn width(&self, font: u32, _: &[u8]) -> Option<f64> {
            self.calls.borrow_mut().push("width");
            assert_eq!(font, 17);
            Some(100.5)
        }
        fn bottom_line(&self) -> Option<i32> {
            self.calls.borrow_mut().push("bottom");
            Some(450)
        }
        fn integer(&self, value: f64) -> i32 {
            self.calls.borrow_mut().push("integer");
            value as i32
        }
    }
    let text = Text {
        calls: RefCell::new(Vec::new()),
        missing: false,
    };
    let layout = CenterNoticeLayout {
        viewport_left: 100,
        viewport_right: 621,
        margin_left: 7,
        margin_right: 13,
        scale_y: 0.5,
        bottom_offset: 4.0,
        line_height: 12.0,
        half_width: 0.5,
        full_alpha: 1.0,
        alpha_scale: 120.0,
        foreground_base: 0x12345600,
        outline_base: 0x87654300,
    };
    let message = Message {
        kind: 1,
        text: b"a\nb\0ignored\n".to_vec(),
        age: 3.0,
        fade_start: 2.0,
        end: 4.0,
        ..Default::default()
    };
    let before = message.clone();
    let draw = center_notice(&message, &layout, &text).unwrap();
    assert_eq!(draw.position, [306.0, 424.0]);
    assert_eq!(draw.font, 17);
    assert_eq!(draw.text, b"a\nb");
    assert_eq!(draw.foreground, 0x1234563c);
    assert_eq!(draw.outline, 0x8765433c);
    assert_eq!(
        *text.calls.borrow(),
        ["integer", "font", "width", "integer", "bottom"]
    );
    assert_eq!(message, before);
    let missing = Text {
        calls: RefCell::new(Vec::new()),
        missing: true,
    };
    assert!(center_notice(&message, &layout, &missing).is_none());
    assert_eq!(*missing.calls.borrow(), ["integer", "font"]);
    assert!(
        center_notice(
            &Message {
                kind: 8,
                ..message.clone()
            },
            &layout,
            &missing
        )
        .is_none()
    );
    let unordered = Message {
        age: f32::NAN,
        ..message
    };
    assert_eq!(
        center_notice(&unordered, &layout, &text)
            .unwrap()
            .foreground,
        0x12345678
    );
}

#[test]
fn owned_notice_dispatch_preserves_renderer_family_and_reports_missing_services() {
    use tsfp_ui_model::messages::Message;
    struct Text(bool);
    impl CenterNoticeText for Text {
        fn fit_font(&self, _: &[u8]) -> Option<u32> {
            self.0.then_some(1)
        }
        fn width(&self, _: u32, _: &[u8]) -> Option<f64> {
            Some(10.0)
        }
        fn bottom_line(&self) -> Option<i32> {
            Some(400)
        }
        fn integer(&self, x: f64) -> i32 {
            x as i32
        }
    }
    impl ProgressNoticeText for Text {
        fn count(&self, text: &[u8]) -> Option<u32> {
            Some(text.len() as u32)
        }
        fn prefix_offset(&self, _: &[u8], n: u32) -> Option<usize> {
            Some(n as usize)
        }
        fn fit_font(&self, _: &[u8]) -> Option<u32> {
            self.0.then_some(7)
        }
        fn wrap(&self, text: &[u8], _: f32) -> Option<Vec<u8>> {
            Some(text.to_vec())
        }
        fn alpha_integer(&self, x: f64) -> i32 {
            x as i32
        }
    }
    impl CornerNoticeText for Text {
        fn fit_corner_font(&self, _: &[u8]) -> Option<u32> {
            self.0.then_some(8)
        }
        fn alpha_integer(&self, x: f64) -> i32 {
            x as i32
        }
    }
    impl SplitNoticeText for Text {
        fn wrap(&self, _: u32, text: &[u8], _: f32) -> Option<Vec<u8>> {
            self.0.then(|| text.to_vec())
        }
        fn bottom_line(&self) -> Option<i32> {
            Some(400)
        }
    }
    let layouts = NoticeLayouts {
        center: CenterNoticeLayout {
            viewport_left: 0,
            viewport_right: 640,
            margin_left: 0,
            margin_right: 0,
            scale_y: 1.0,
            bottom_offset: 4.0,
            line_height: 12.0,
            half_width: 0.5,
            full_alpha: 1.0,
            alpha_scale: 120.0,
            foreground_base: 0,
            outline_base: 0,
        },
        progress: ProgressNoticeLayout {
            viewport_left: 0,
            viewport_right: 640,
            viewport_bottom: 480,
            margin_left: 0,
            margin_right: 0,
            margin_bottom: 0,
            effective_players: 1,
            scale: [1.0; 2],
            reveal_rate: 1.0,
            full_alpha: 1.0,
            alpha_scale: 120.0,
            foreground_base: 0,
            outline_base: 0,
        },
        corner: CornerNoticeLayout {
            viewport_left: 0,
            margin_left: 0,
            viewport_bottom: 480,
            line_height: 12.0,
            height_factor: 1.0,
            baseline_offset: 0.0,
            full_alpha: 1.0,
            alpha_scale: 120.0,
            colour_base: 0,
        },
        split: SplitNoticeLayout {
            viewport_left: 0,
            viewport_right: 640,
            margin_left: 0,
            margin_right: 0,
            mode: 0,
            level_players: 1,
            alternate_context: false,
            normal_font: 9,
            alternate_font: 12,
            width_factor: 1.0,
            foreground: 0,
            outline: 0,
        },
    };
    let mut queue = OwnedMessageQueue::default();
    for (index, kind) in [1, 7, 8, 9].into_iter().enumerate() {
        queue.ring.entries[index] = Message {
            kind,
            text: b"payload".to_vec(),
            age: 0.0,
            fade_start: 1.0,
            end: 2.0,
            weight: 1,
            ..Default::default()
        };
    }
    let before = queue.clone();
    let policy = MessagePolicy {
        limits: [100; 14],
        conflicts: vec![],
    };
    let mut progress = 1;
    let mut draws = vec![];
    queue.replay_native(
        &policy,
        false,
        true,
        &mut progress,
        || true,
        |entry, _, _, progress| {
            draws.push(
                present_notice(entry, progress, &layouts, &Text(true))
                    .unwrap()
                    .unwrap(),
            )
        },
    );
    assert!(matches!(draws[0], NoticeDraw::Outlined(_)));
    assert!(matches!(&draws[1], NoticeDraw::Outlined(draw) if draw.text == b"payload"));
    assert!(matches!(draws[2], NoticeDraw::Corner(_)));
    assert!(matches!(&draws[3], NoticeDraw::Split(draw) if draw.centered));
    assert_eq!(progress, 2);
    assert_eq!(queue, before);
    for kind in [1, 7, 8, 9] {
        let entry = Message {
            kind,
            text: b"payload".to_vec(),
            ..Default::default()
        };
        assert_eq!(
            present_notice(&entry, 2, &layouts, &Text(false)),
            Err("native notice metric unavailable")
        );
    }
    assert_eq!(
        present_notice(
            &Message {
                kind: 2,
                ..Default::default()
            },
            0,
            &layouts,
            &Text(false)
        ),
        Ok(None)
    );
}
