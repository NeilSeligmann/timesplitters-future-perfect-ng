use tsfp_gameplay::char_tick_entity::{EntityCtx, EntityLeaf, EntityWorld, tick};
struct World {
    i: Vec<u32>,
    calls: Vec<u32>,
}
impl EntityWorld for World {
    fn leaf(&mut self, l: EntityLeaf, c: &mut EntityCtx) -> u32 {
        self.calls.push(l.original_address());
        match l {
            EntityLeaf::Resolve => self.i[2],
            EntityLeaf::Owner => self.i[3],
            EntityLeaf::MoveAllowed => self.i[4],
            EntityLeaf::CanExit => self.i[5],
            EntityLeaf::SetRoute(r) => {
                self.calls.push(r);
                self.i[6]
            }
            EntityLeaf::Attached => self.i[7] & self.i[8],
            EntityLeaf::Classify => self.i[9],
            EntityLeaf::Status => self.i[10],
            EntityLeaf::Kind => self.i[12],
            EntityLeaf::Route => 3,
            EntityLeaf::Locate => 3,
            EntityLeaf::Node => {
                c.object_room = self.i[18] as i32;
                0
            }
            _ => 0,
        }
    }
    fn control(&mut self, c: &mut EntityCtx, s: f32) -> i32 {
        self.calls.extend([0x17e010, c.state, s.to_bits()]);
        self.i[13] as i32
    }
    fn scale_speed(&mut self, v: f32) -> f32 {
        self.calls.extend([0x172cd0, v.to_bits()]);
        0.5
    }
    fn class(&mut self, m: u64) {
        self.calls.extend([0x17570, m as u32, (m >> 32) as u32]);
    }
}
#[test]
fn full_entity_tick_matches_original() {
    let mut count = 0;
    for line in include_str!("data/t1877/entity.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = EntityCtx {
            state: i[0],
            pending: i[1] as i32,
            previous: 9,
            state_time: 0.3,
            auxiliary_flags: i[16] as u8,
            drain: 0.25,
            tick_rate: 60,
            delta_ticks: 1.0,
            target: i[17],
            entity_flags: i[11],
            entity_speed: f32::from_bits(i[14]),
            settle_timer: f32::from_bits(i[15]),
            origin: 7,
            object_room: 9,
        };
        let mut w = World {
            i,
            calls: Vec::new(),
        };
        tick(&mut c, &mut w);
        let mut actual = vec![
            c.state,
            c.pending as u32,
            c.previous,
            c.state_time.to_bits(),
            c.drain.to_bits(),
            c.target,
            c.settle_timer.to_bits(),
            c.origin as u32,
            c.entity_flags,
            c.entity_speed.to_bits(),
            c.object_room as u32,
        ];
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 2600);
}
