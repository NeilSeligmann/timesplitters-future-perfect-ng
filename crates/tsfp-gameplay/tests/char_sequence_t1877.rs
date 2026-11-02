//! State and selection-call differential against original 0x262720 controlled worlds.
use tsfp_gameplay::char_sequence::{self, SequenceCtx, SequenceWorld, Step};
struct World {
    input: Vec<u32>,
    calls: Vec<u32>,
    counts: [u32; 12],
}
impl SequenceWorld for World {
    fn aware(&mut self) -> bool {
        {
            self.counts[7] += 1;
            self.input[8] != 0
        }
    }
    fn tracked_target(&mut self) -> bool {
        self.input[14] != 0
    }
    fn change_class(&mut self, mask: u64) {
        self.calls
            .extend([0x17570, mask as u32, (mask >> 32) as u32]);
    }
    fn resolve_node(&mut self, _tag: u32) -> i32 {
        let n = self.input[15] as i32;
        self.counts[1] += if n < 0 { 1 } else { 2 };
        if n < 0 { 77 } else { n }
    }
    fn despawn(&mut self) {
        self.counts[0] += 1;
    }
    fn animation_interrupted(&mut self) -> bool {
        self.counts[8] += 1;
        self.input[9] != 0
    }
    fn plays(&mut self, _id: u32) -> bool {
        self.counts[9] += 1;
        self.input[10] != 0
    }
    fn finished(&mut self) -> bool {
        self.counts[10] += 1;
        self.input[11] != 0
    }
    fn play(&mut self, id: u32, blend: f32, speed: f32) {
        self.calls
            .extend([0x5F460, id, blend.to_bits(), speed.to_bits()]);
    }
    fn walk_finished(&mut self, _node: i32) -> bool {
        self.counts[2] += 1;
        self.input[12] != 0
    }
    fn walk(&mut self) -> bool {
        for n in [3, 4, 5, 11] {
            self.counts[n] += 1;
        }
        self.input[13] != 0
    }
    fn finish_sequence(&mut self) {
        self.counts[6] += 1;
        self.counts[3] += 1;
    }
}
#[test]
fn sequence_tick_matches_original_state_and_selection_calls() {
    let mut count = 0;
    for line in include_str!("data/t1877/sequence_tick.golden").lines() {
        let (input, expected) = line[2..].split_once(" = ").unwrap();
        let hex = |text: &str| {
            text.split_whitespace()
                .map(|s| u32::from_str_radix(s, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = hex(input);
        let e = hex(expected);
        let mut ctx = SequenceCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: i[2],
            state_time: 1.,
            flags_byte: i[3] as u8,
            speed_scale: f32::from_bits(i[4]),
            step: i[5],
            node: 99,
            walk_timer: 19,
            walk_progress: 23,
            flags: i[16],
            parked_mask: 0x45600000123,
            class_mask: i[17] as u64 | ((i[18] as u64) << 32),
            exit_mask: i[19] as u64 | ((i[20] as u64) << 32),
            end_animation: i[21],
        };
        let steps = [Step {
            kind: i[6],
            argument: 0,
            value: i[7],
        }; 3];
        let mut world = World {
            input: i,
            calls: Vec::new(),
            counts: [0; 12],
        };
        char_sequence::tick(&mut ctx, &steps, 1.0 / 60.0, &mut world).unwrap();
        let got = [
            ctx.state,
            ctx.pending as u32,
            ctx.previous,
            ctx.state_time.to_bits(),
            ctx.speed_scale.to_bits(),
            ctx.step,
            ctx.node as u32,
            ctx.walk_timer,
            ctx.walk_progress,
            ctx.flags,
            ctx.parked_mask as u32,
            (ctx.parked_mask >> 32) as u32,
        ];
        assert_eq!(got.as_slice(), &e[..12], "state {line}");
        assert_eq!(
            world.counts.as_slice(),
            &e[12..24],
            "engine call coverage {line}"
        );
        assert_eq!(world.calls.as_slice(), &e[25..], "selection calls {line}");
        count += 1;
    }
    assert_eq!(count, 1800);
}
#[test]
fn sequence_parser_bounds_and_relocation() {
    let mut blob = vec![0; 0x140];
    blob[..4].copy_from_slice(b"SETU");
    for (at, w) in [
        (8, 0x120u32),
        (0xCC, 0x110),
        (0xD0, 2),
        (0xD4, 8),
        (0x110, 0xFFFFFFFF),
        (0x114, 1),
        (0x118, 0xDEADBEEF),
        (0x11C, 1),
        (0x120, 8),
        (0x128, 0x25),
        (0x12C, 18),
    ] {
        blob[at..at + 4].copy_from_slice(&w.to_le_bytes());
    }
    let parsed = char_sequence::parse(&blob).unwrap();
    assert_eq!(parsed[0][0].value, 0x25);
    assert_eq!(parsed[1][0].kind, 18);
    blob[0x11C..0x120].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        char_sequence::parse(&blob),
        Err(char_sequence::SequenceError::StepsOutsideEntry)
    );
    assert_eq!(
        char_sequence::parse(b"SETU"),
        Err(char_sequence::SequenceError::ShortEntry)
    );
    assert_eq!(
        char_sequence::parse(b"other"),
        Err(char_sequence::SequenceError::NotSetu)
    );
    assert_eq!(
        char_sequence::SequenceError::StepsOutsideEntry.to_string(),
        "sequence steps outside SETU entry"
    );
}
