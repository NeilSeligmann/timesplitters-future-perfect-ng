use tsfp_engine_anim::binding::{BindingWorld, LevelBindings, RESIDENT_SETS};
struct World {
    seed: u32,
    set: usize,
    entry: usize,
    calls: Vec<u32>,
    loaded: Vec<(u16, String)>,
}
fn checksum(s: &str) -> u32 {
    s.bytes().map(u32::from).sum()
}
impl BindingWorld for World {
    fn resident(&mut self, pak: &str) -> bool {
        let index = self.set;
        self.set += 1;
        self.entry = 0;
        let sum = checksum(pak);
        self.calls.push(sum);
        let yes = self.seed == 1 || (self.seed > 1 && (sum + self.seed) % 7 < 3);
        if yes {
            self.calls.extend([
                0x5B980,
                RESIDENT_SETS[index].table,
                RESIDENT_SETS[index].entries.len() as u32,
            ]);
        }
        yes
    }
    fn load_clip(&mut self, path: &str, id: u16) -> i16 {
        let e = RESIDENT_SETS[self.set - 1].entries[self.entry];
        self.entry += 1;
        self.calls
            .extend([u32::from(id), e.policy[0], e.policy[1], checksum(path)]);
        self.loaded.push((id, path.into()));
        (self.loaded.len() % 32000) as i16
    }
}
#[test]
fn resident_binding_order_matches_original() {
    assert_eq!(RESIDENT_SETS.len(), 103);
    for line in include_str!("data/t1877_binding.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let seed = u32::from_str_radix(i, 16).unwrap();
        let expected = e
            .split_whitespace()
            .map(|v| u32::from_str_radix(v, 16).unwrap())
            .collect::<Vec<_>>();
        let mut w = World {
            seed,
            set: 0,
            entry: 0,
            calls: Vec::new(),
            loaded: Vec::new(),
        };
        let mut b = LevelBindings::default();
        b.load_resident(&mut w);
        assert_eq!(w.calls, expected, "seed {seed}");
        let mut final_paths = std::collections::BTreeMap::new();
        for (id, path) in w.loaded {
            final_paths.insert(id, path);
        }
        assert_eq!(
            b.ids
                .iter()
                .map(|(id, c)| (*id, c.path.clone()))
                .collect::<std::collections::BTreeMap<_, _>>(),
            final_paths
        );
        assert_eq!(w.set, 103);
    }
}

#[test]
fn registered_real_stream_zone_tables_match_original() {
    use tsfp_engine_anim::binding::{zone_tag, ZoneRow};
    let mut cases = 0;
    for line in include_str!("data/t1877_stream_zone.golden").lines() {
        let (i, e) = line[2..].split_once(" = ").unwrap();
        let i = i
            .split_whitespace()
            .map(|v| u32::from_str_radix(v, 16).unwrap())
            .collect::<Vec<_>>();
        let mut cursor = 1;
        let channels: [Vec<ZoneRow>; 2] = std::array::from_fn(|_| {
            let count = i[cursor] as usize;
            cursor += 1;
            (0..count)
                .map(|_| {
                    let row = ZoneRow {
                        tag: i[cursor],
                        mask: i[cursor + 1],
                    };
                    cursor += 2;
                    row
                })
                .collect()
        });
        let actual = zone_tag([&channels[0], &channels[1]], i[0] as u8).unwrap_or(u32::MAX);
        assert_eq!(actual, u32::from_str_radix(e, 16).unwrap(), "{line}");
        cases += 1;
    }
    assert_eq!(cases, 512);
}

#[test]
fn real_level_stream_plans_and_post_install_bindings() {
    use tsfp_engine_anim::binding::{plan_stream, StreamArchive, STREAMED_SETS};
    struct InstalledWorld {
        loaded: Vec<(u16, String)>,
    }
    impl BindingWorld for InstalledWorld {
        fn resident(&mut self, _: &str) -> bool {
            false
        }
        fn load_clip(&mut self, path: &str, id: u16) -> i16 {
            self.loaded.push((id, path.into()));
            self.loaded.len() as i16
        }
    }
    let mut count = 0;
    for line in include_str!("data/t1877_stream_plan.golden").lines() {
        let (input, expected) = line[2..].split_once(" = ").unwrap();
        let mut input = input.split_whitespace();
        let number = |s: &str| u32::from_str_radix(s, 16).unwrap();
        let zone = number(input.next().unwrap());
        let channels: [Vec<StreamArchive>; 2] = std::array::from_fn(|_| {
            let n = number(input.next().unwrap());
            (0..n)
                .map(|_| {
                    let mut row = input.next().unwrap().split(',');
                    let tag = number(row.next().unwrap());
                    let mask = number(row.next().unwrap());
                    let callback = number(row.next().unwrap());
                    let pak = row.next().unwrap();
                    StreamArchive {
                        tag,
                        mask,
                        callback,
                        pak: if pak == "-" {
                            None
                        } else {
                            Some(Box::leak(pak.to_string().into_boxed_str()))
                        },
                    }
                })
                .collect()
        });
        assert!(input.next().is_none());
        let expected = expected.split_whitespace().map(number).collect::<Vec<_>>();
        let plan = plan_stream([&channels[0], &channels[1]], zone as u8);
        assert_eq!(
            plan.map(|p| p.tag).unwrap_or(u32::MAX),
            expected[0],
            "{line}"
        );
        if let Some(plan) = plan {
            for channel in 0..2 {
                assert_eq!(
                    plan.channels[channel],
                    if expected[channel + 1] == u32::MAX {
                        None
                    } else {
                        Some(channels[channel][expected[channel + 1] as usize])
                    }
                );
            }
            let mut world = InstalledWorld { loaded: Vec::new() };
            let mut b = LevelBindings::begin_level(&mut world);
            assert!(world.loaded.is_empty()); // A request plan has no installation side effects.
            for row in plan.channels.into_iter().flatten() {
                let known = STREAMED_SETS.iter().find(|s| Some(s.set.pak) == row.pak);
                let old = world.loaded.len();
                assert_eq!(b.installed_stream(row, &mut world), known.is_some());
                if let Some(s) = known {
                    let paths = s
                        .set
                        .entries
                        .iter()
                        .map(|e| (e.id, format!("{}{}.raw", s.set.base, e.name)))
                        .collect::<Vec<_>>();
                    assert_eq!(&world.loaded[old..], paths);
                    for (id, path) in paths {
                        assert_eq!(b.ids[&id].path, path);
                    }
                } else {
                    assert_eq!(world.loaded.len(), old);
                }
            }
        }
        count += 1;
    }
    assert_eq!(count, 512);
}

#[test]
fn level_animation_loader_residency_then_binding_matches_original() {
    struct LevelWorld {
        installed: Vec<String>,
        calls: Vec<u32>,
        set: usize,
        entry: usize,
    }
    impl BindingWorld for LevelWorld {
        fn resident(&mut self, pak: &str) -> bool {
            self.set += 1;
            self.entry = 0;
            self.installed.iter().any(|p| p == pak)
        }
        fn load_clip(&mut self, path: &str, id: u16) -> i16 {
            let e = RESIDENT_SETS[self.set - 1].entries[self.entry];
            self.entry += 1;
            self.calls
                .extend([u32::from(id), e.policy[0], e.policy[1], checksum(path)]);
            (self.calls.len() / 4) as i16
        }
    }
    for line in include_str!("data/t1877_level_binding.golden").lines() {
        let (level, expected) = line[2..].split_once(" = ").unwrap();
        let level = i32::from_str_radix(level, 16).unwrap();
        let mut w = LevelWorld {
            installed: tsfp_engine_anim::loaders::pak_list(level, 10, 0, 0),
            calls: Vec::new(),
            set: 0,
            entry: 0,
        };
        let b = LevelBindings::begin_level(&mut w);
        let expected = expected
            .split_whitespace()
            .map(|v| u32::from_str_radix(v, 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(w.calls.len(), expected.len(), "level {level} length");
        for (n, (a, b)) in w.calls.iter().zip(&expected).enumerate() {
            assert_eq!(a, b, "level {level} word {n}");
        }
        assert!(!b.ids.is_empty());
    }
}
