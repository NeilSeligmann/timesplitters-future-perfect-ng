//! T1846 batch 8: group path slot update (0x33BFA0 and callees) against emulated-original golden vectors.
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::group_paths::*;

const ENT: u32 = 0x100_0000;
const TYP: u32 = 0x101_0000;
const AREA: u32 = 0x102_0000;
const NOP_STUB: u32 = 0x10B_0020;
/// The generator records the stack argument (area id) here.
const AREA_ARG: u32 = 0x10C_0000;

fn dword(case: &Case, address: u32) -> u32 {
    case.input(address)
}
fn word(case: &Case, address: u32) -> i16 {
    (case.input(address) & 0xFFFF) as i16
}
fn word_at(case: &Case, address: u32) -> i16 {
    let value = case.input(address & !3);
    (value >> (8 * (address & 2))) as i16
}
fn slots(case: &Case, address: u32) -> [i16; SLOTS] {
    let mut out = [0; SLOTS];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = word_at(case, address + 2 * i as u32);
    }
    out
}

struct Hooks {
    lcg: Lcg,
    deep_calls: u32,
}
impl PathHooks for Hooks {
    fn rng(&mut self) -> u32 {
        self.lcg.next()
    }
    fn deep_member(&mut self, _member: &mut Member, _out: &mut Vec<i32>) {
        self.deep_calls += 1;
    }
}

fn id_rows(case: &Case, base: u32, count: u32) -> Vec<IdRow> {
    (0..count)
        .map(|i| {
            let at = base + 32 * i;
            IdRow {
                key: dword(case, at) as i32,
                flags: dword(case, at + 4),
                word_8: word(case, at + 8),
            }
        })
        .collect()
}

fn build(case: &Case) -> PathWorld {
    let n_main = dword(case, 0x7B6664);
    let n_t34 = dword(case, 0x7B65C0);
    let n_groups = dword(case, 0x7B666C);
    let mut ids = IdTables {
        main: id_rows(case, dword(case, 0x715F2C), n_main),
        t34: id_rows(case, dword(case, 0x715F34), n_t34),
        t38: id_rows(case, dword(case, 0x715F38), 4),
        ..IdTables::default()
    };
    // t38 rows beyond the generator's count are zero memory; keep only the written ones.
    ids.t38.retain(|row| row.key != 0);
    let edges_base = dword(case, 0x715F30);
    for i in 0..14u32 {
        let at = edges_base + 20 * i;
        ids.edges.push(Edge {
            flags: dword(case, at),
            a: dword(case, at + 4) as i32,
            b: dword(case, at + 8) as i32,
        });
    }
    let group_table = dword(case, 0x7B6770);
    for g in 0..n_groups {
        let at = dword(case, group_table + 4 * g);
        let (skipped, nb, nc) = (dword(case, at), dword(case, at + 4), dword(case, at + 8));
        let list = |first: u32, n: u32| {
            (0..n)
                .map(|k| dword(case, at + 0x18 + 4 * (first + k)))
                .collect::<Vec<_>>()
        };
        ids.groups.push(Group {
            skipped,
            list_b: list(skipped, nb),
            list_c: list(skipped + nb, nc),
        });
    }
    let entries = (0..dword(case, 0x78CCF0))
        .map(|i| {
            let at = ENT + 0x10 * i;
            PathEntry {
                id: dword(case, at) as i32,
                kind: word(case, at + 8),
                slot_count: word_at(case, at + 0xA),
            }
        })
        .collect();
    let typed_rows = (0..dword(case, 0x78CCF8))
        .map(|i| {
            let at = TYP + 0x20 * i;
            TypedRow {
                first: dword(case, at) as i32,
                kind: word(case, at + 4),
                slots: slots(case, at + 0xC),
            }
        })
        .collect();
    let area_rows = (0..4u32)
        .map(|i| AreaRow {
            section_20: slots(case, AREA + 0x60 * i + 0x20),
            section_40: slots(case, AREA + 0x60 * i + 0x40),
        })
        .collect();
    let object_base = dword(case, 0x108_0084);
    let objects = (0..dword(case, 0x108_0080))
        .map(|i| {
            let at = object_base + 0x28 * i;
            ObjectRec {
                id: dword(case, at + 4) as i32,
                path: dword(case, at + 0xC) as i32,
                state: dword(case, at + 0x1C) as i32,
                callback_makes_ready: dword(case, at + 0x14) != NOP_STUB,
            }
        })
        .collect();
    let path_base = dword(case, 0x108_00B0);
    let paths = (0..3u32)
        .map(|i| {
            let at = path_base + 0x58 * i;
            let mut path = PathRec {
                count: dword(case, at + 4) as i32,
                ..PathRec::default()
            };
            for j in 0..SLOTS as u32 {
                path.ids[j as usize] = dword(case, at + 8 + 4 * j) as i32;
                path.blocked[j as usize] = dword(case, at + 0x30 + 4 * j) as i32;
            }
            path
        })
        .collect();
    let lists_base = dword(case, 0x7789F8);
    let member_lists = (0..3u32)
        .map(|i| {
            let header = dword(case, lists_base + 4 * i);
            (header != 0).then(|| {
                let (base, count) = (dword(case, header), dword(case, header + 4));
                (0..count)
                    .map(|k| {
                        let at = base + 12 * k;
                        let (game, info) = (dword(case, at), dword(case, at + 4));
                        let link = dword(case, game + 0x7C);
                        Member {
                            kind: dword(case, info + 8) as i32,
                            object_id: dword(case, at + 8) as i32,
                            game_flags_28: dword(case, game + 0x28),
                            game_link_value: dword(case, link + 4) as i32,
                        }
                    })
                    .collect()
            })
        })
        .collect();
    PathWorld {
        entries,
        typed_rows,
        area_rows,
        member_lists,
        objects,
        paths,
        ids,
    }
}

#[test]
fn update_area_paths_matches_original() {
    let cases = parse(include_str!("data/t1846/group_paths_33bfa0.golden"));
    assert!(cases.len() >= 100);
    let (
        mut draws_seen,
        mut empty_lists,
        mut kind_one_two,
        mut kind_three_four,
        mut replicated,
        mut padded,
    ) = (0, 0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = build(case);
        let area_id = case.input(AREA_ARG) as i32;
        let mut hooks = Hooks {
            lcg: Lcg {
                state: u64::from(case.input(0x4E7950)) | (u64::from(case.input(0x4E7954)) << 32),
                draws: 0,
            },
            deep_calls: 0,
        };
        let before = world.clone();
        update_area_paths(&mut world, &mut hooks, area_id).expect("update");
        assert_eq!(hooks.deep_calls, 0);
        for (i, row) in world.typed_rows.iter().enumerate() {
            let at = TYP + 0x20 * i as u32;
            assert_eq!(
                row.slots,
                slots_out(case, at + 0xC),
                "case {n} typed row {i} slots"
            );
            let kind_word = (case.input(at + 4) & 0xFFFF_0000) | (row.kind as u16 as u32);
            assert_eq!(
                kind_word,
                case.output(at + 4),
                "case {n} typed row {i} kind"
            );
            assert_eq!(
                row.first as u32,
                case.output(at),
                "case {n} typed row {i} first"
            );
        }
        let lcg_state = hooks.lcg.state;
        assert_eq!(lcg_state as u32, case.output(0x4E7950), "case {n} rng low");
        assert_eq!(
            (lcg_state >> 32) as u32,
            case.output(0x4E7954),
            "case {n} rng high"
        );
        assert_eq!(hooks.lcg.draws, case.output(0x7497C8), "case {n} rng draws");
        let object_base = 0x109_0000;
        for (i, object) in world.objects.iter().enumerate() {
            assert_eq!(
                object.state as u32,
                case.output(object_base + 0x28 * i as u32 + 0x1C),
                "case {n} object {i} state"
            );
        }
        for (list, members) in world.member_lists.iter().enumerate() {
            for (slot, member) in members.iter().flatten().enumerate() {
                let game = 0x106_0000 + 0x100 * (list * 8 + slot) as u32;
                assert_eq!(
                    member.game_flags_28,
                    case.output(game + 0x28),
                    "case {n} game flags {list}/{slot}"
                );
            }
        }
        draws_seen += u32::from(hooks.lcg.draws > 0);
        empty_lists += world
            .member_lists
            .iter()
            .filter(|l| l.as_ref().is_none_or(|m| m.is_empty()))
            .count();
        kind_one_two += before
            .member_lists
            .iter()
            .flatten()
            .flatten()
            .filter(|m| m.kind == 1 || m.kind == 2)
            .count();
        kind_three_four += before
            .member_lists
            .iter()
            .flatten()
            .flatten()
            .filter(|m| m.kind == 3 || m.kind == 4)
            .count();
        replicated += before.entries.iter().filter(|e| e.slot_count > 0).count();
        padded += world
            .typed_rows
            .iter()
            .filter(|r| r.slots.contains(&-1))
            .count();
    }
    assert!(draws_seen > 20, "random path covered {draws_seen}");
    assert!(
        empty_lists > 20
            && kind_one_two > 20
            && kind_three_four > 20
            && replicated > 20
            && padded > 20
    );
}

fn slots_out(case: &Case, address: u32) -> [i16; SLOTS] {
    let mut out = [0; SLOTS];
    for (i, slot) in out.iter_mut().enumerate() {
        let at = address + 2 * i as u32;
        *slot = (case.output(at & !3) >> (8 * (at & 2))) as i16;
    }
    out
}

#[test]
fn lcg_and_binary_search_semantics() {
    let mut lcg = Lcg { state: 1, draws: 0 };
    assert_eq!(lcg.next(), 0x343FD + 0x26_9EC3);
    assert_eq!(lcg.draws, 1);
    let mut tables = IdTables::default();
    for key in [1001, 1005, 1010, 1020] {
        tables.main.push(IdRow {
            key,
            flags: 0,
            word_8: 0,
        });
    }
    assert_eq!(tables.row_index(1010), 2);
    assert_eq!(tables.row_index(1001), 0);
    assert_eq!(tables.row_index(1002), -1);
    assert_eq!(tables.key_of_row(3), 1020);
    assert_eq!(tables.key_of_row(4), -1);
    tables.t34.push(IdRow {
        key: 0x2400,
        flags: 0,
        word_8: 0,
    });
    assert_eq!(tables.row_index(0x2400), 0x1F40);
    assert_eq!(tables.key_of_row(0x1F40), 0x2400);
}
