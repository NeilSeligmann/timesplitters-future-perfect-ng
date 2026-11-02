use tsfp_gameplay::char_class::{ClassTransition, ClassTransitionWorld, apply_class_transition};

#[derive(Default)]
struct World {
    wake: bool,
    calls: Vec<u32>,
    mutate: Option<(u8, u64)>,
}
impl ClassTransitionWorld for World {
    fn leaving(&mut self, state: &mut ClassTransition, _: u64, _: u64) {
        self.calls.push(0x158D40);
        if let Some((mode, requested)) = self.mutate {
            state.mode = mode;
            state.requested = requested;
        }
    }
    fn dormant(&mut self, _: &mut ClassTransition) {
        self.calls.push(0x15BCC0);
    }
    fn wake(&mut self, _: &mut ClassTransition) {
        self.wake = true;
    }
    fn entered(&mut self, _: &mut ClassTransition) {
        self.calls.push(0x27B8B0);
    }
}
#[test]
fn class_application_matches_original() {
    let sentinel = 0x5A5A5A5A;
    for line in include_str!("data/t1877/class_transition.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|x| u32::from_str_radix(x, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mask = |lo: u32, hi: u32| u64::from(lo) | (u64::from(hi) << 32);
        let mut s = ClassTransition {
            mode: i[0] as u8,
            story_mask: mask(i[1], i[2]),
            other_mask: mask(i[3], i[4]),
            requested: mask(i[5], i[6]),
            previous: mask(sentinel, sentinel),
            transition_time: f32::from_bits(sentinel),
            clock: f32::from_bits(sentinel),
            auxiliary_time: f32::from_bits(sentinel),
            auxiliary_state: sentinel,
            flags: i[7],
        };
        let mut w = World {
            mutate: if i[8] != 0 {
                Some((i[9] as u8, mask(i[10], i[11])))
            } else {
                None
            },
            ..World::default()
        };
        apply_class_transition(&mut s, &mut w);
        let actual = [
            s.story_mask as u32,
            (s.story_mask >> 32) as u32,
            s.other_mask as u32,
            (s.other_mask >> 32) as u32,
            s.previous as u32,
            (s.previous >> 32) as u32,
            s.transition_time.to_bits(),
            s.clock.to_bits(),
            s.auxiliary_time.to_bits(),
            s.auxiliary_state,
            s.flags,
            if w.wake { 8 } else { 7 },
            if w.wake { 0x02D10000 } else { sentinel },
        ];
        assert_eq!(actual.as_slice(), e, "{line}");
        if s.previous == mask(sentinel, sentinel) {
            assert!(w.calls.is_empty());
        } else {
            assert_eq!(w.calls.first(), Some(&0x158D40));
            assert_eq!(w.calls.last(), Some(&0x27B8B0));
        }
    }
}
