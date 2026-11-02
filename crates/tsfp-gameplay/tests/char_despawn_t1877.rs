use tsfp_gameplay::char_despawn::{DespawnCtx, DespawnWorld, GroupCounts, Release, despawn};
struct World {
    valid: bool,
    node_flags: u32,
    calls: Vec<u32>,
}
impl DespawnWorld for World {
    fn release(&mut self, r: Release, _: &mut DespawnCtx) {
        self.calls.push(r.original_address());
        if let Release::RecordEvent(e) = r {
            self.calls.push(e);
        }
    }
    fn resolve_node(&mut self, t: i16) -> i32 {
        self.calls.extend([0x9cc40, t as i32 as u32]);
        3
    }
    fn node_flags(&mut self, n: i32) -> &mut u32 {
        assert_eq!(n, 3);
        &mut self.node_flags
    }
    fn registered_handle(&mut self, h: u32) -> u32 {
        if self.valid { h } else { 0 }
    }
}
#[test]
fn actor_teardown_matches_original_with_real_name_clear() {
    let mut count = 0;
    for line in include_str!("data/t1877/despawn.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let words = |s: &str| {
            s.split_whitespace()
                .map(|v| u32::from_str_radix(v, 16).unwrap())
                .collect::<Vec<_>>()
        };
        let i = words(i);
        let e = words(e);
        let mut c = DespawnCtx {
            object: 0x2d00000,
            target: 17,
            record: 0x2d20000,
            story_mask: 0x200000001,
            other_mask: 0x400000003,
            object_flags: i[8],
            object_speed: 1.0,
            flags: if i[1] != 0 { 0x400000 } else { 0 },
            node: 5,
            name: [0xab; 32],
            group_present: i[0] != 0,
            group: GroupCounts {
                active: 2,
                removed: 3,
            },
            active_count: 4,
            story_mode: i[2] != 0,
            record_event: i[7],
            reserved_tag: if i[3] != 0 { Some(i[4] as i16) } else { None },
            entity: i[5],
        };
        let mut w = World {
            valid: i[6] != 0,
            node_flags: u32::MAX,
            calls: Vec::new(),
        };
        despawn(&mut c, &mut w);
        let mut actual = vec![
            c.object,
            c.target,
            c.story_mask as u32,
            (c.story_mask >> 32) as u32,
            c.other_mask as u32,
            (c.other_mask >> 32) as u32,
            c.object_flags,
            c.object_speed.to_bits(),
        ];
        actual.extend(
            c.name
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
        );
        actual.extend([
            c.node as u32,
            c.record,
            if c.group_present { 0x2d30000 } else { 0 },
            c.group.active,
            c.group.removed,
            c.active_count,
            if c.reserved_tag.is_some() {
                0x2d40000
            } else {
                0
            },
            w.node_flags,
            c.entity,
        ]);
        actual.extend(w.calls);
        assert_eq!(actual, e, "{line}");
        count += 1;
    }
    assert_eq!(count, 800);
}
