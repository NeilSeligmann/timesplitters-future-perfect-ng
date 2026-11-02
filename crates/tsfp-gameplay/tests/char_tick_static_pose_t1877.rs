//! T1877: static pose class tick `0x2648C0` against the ORIGINAL (golden `tests/data/t1877/static_pose.golden`).
use tsfp_gameplay::char_tick_static_pose::{StaticPoseCtx, StaticPoseWorld, tick};

fn hex(token: &str) -> u32 {
    u32::from_str_radix(token, 16).unwrap()
}

#[derive(Default)]
struct World {
    plays: bool,
    poses: Vec<(i32, u32)>,
    made: Vec<(u32, u32, u32)>,
    settled: u32,
}

impl StaticPoseWorld for World {
    fn aware(&mut self) {}
    fn pose(&mut self, id: i32, time: f32) {
        self.poses.push((id, time.to_bits()));
    }
    fn plays(&self, _id: i32) -> bool {
        self.plays
    }
    fn play(&mut self, id: i32, blend: f32, speed: f32) {
        self.made
            .push((id as u32, blend.to_bits(), speed.to_bits()));
    }
    fn settle(&mut self) {
        self.settled += 1;
    }
}

#[test]
fn static_pose_tick_matches_the_original() {
    let text = include_str!("data/t1877/static_pose.golden");
    let (mut cases, mut played, mut with_id) = (0, 0, 0);
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!((t[0], t[11]), ("S", "="));
        let mut ctx = StaticPoseCtx {
            state: hex(t[1]),
            pending: hex(t[2]) as i32,
            previous: hex(t[3]),
            state_time: 1.0,
            flags_byte: hex(t[4]) as u8,
            speed_scale: f32::from_bits(hex(t[5])),
            pose_time: f32::from_bits(hex(t[10])),
            record_pose_id: (hex(t[6]) != 0).then_some(hex(t[7]) as u16 as i16),
            object_flags: hex(t[9]),
        };
        let mut world = World {
            plays: hex(t[8]) != 0,
            ..World::default()
        };
        tick(&mut ctx, &mut world);
        let got = [
            ctx.state,
            ctx.previous,
            ctx.pending as u32,
            ctx.state_time.to_bits(),
            ctx.speed_scale.to_bits(),
            ctx.object_flags,
        ];
        let want: Vec<u32> = (12..18).map(|i| hex(t[i])).collect();
        assert_eq!(got.to_vec(), want, "{line}");
        let n_poses = hex(t[18]) as usize;
        assert_eq!(world.poses.len(), n_poses, "{line}");
        for (i, pose) in world.poses.iter().enumerate() {
            assert_eq!(pose.1, hex(t[20 + 2 * i]), "pose time {line}");
        }
        let at = 19 + 2 * n_poses;
        let n_made = hex(t[at]) as usize;
        let want_made: Vec<(u32, u32, u32)> = (0..n_made)
            .map(|i| {
                (
                    hex(t[at + 1 + 3 * i]),
                    hex(t[at + 2 + 3 * i]),
                    hex(t[at + 3 + 3 * i]),
                )
            })
            .collect();
        assert_eq!(world.made, want_made, "{line}");
        assert_eq!(world.settled, hex(t[at + 1 + 3 * n_made]), "{line}");
        cases += 1;
        played += n_made;
        with_id += usize::from(world.made.iter().any(|m| m.0 != u32::MAX));
    }
    eprintln!("MEASURED static pose tick: {cases} cases, {played} plays, {with_id} with a real id");
    assert!(cases >= 800 && played > 50 && with_id > 20);
}
