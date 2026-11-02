//! T1846 batch 8: sine table, follow velocity and level view adapters.
mod common;
use common::parse;
use tsfp_gameplay::story_events::hooks_story::{HookView, NpcView};
use tsfp_gameplay::story_events::level_roles::*;

#[test]
fn sine_table_matches_original_init_bit_for_bit() {
    let cases = parse(include_str!("data/t1846/sine_table_153c20.golden"));
    assert_eq!(cases.len(), 1);
    let table = build_sine_table();
    assert_eq!(table.len(), SINE_ENTRIES);
    for (index, value) in table.iter().enumerate() {
        let want = cases[0].output(SINE_TABLE_ADDRESS + 4 * index as u32);
        assert_eq!(value.to_bits(), want, "entry {index:#x}");
    }
}

#[test]
fn sine_table_readers() {
    let table = build_sine_table();
    assert_eq!(table_sin(&table, 0), 0.0);
    assert_eq!(table_sin(&table, 0x200), 1.0);
    assert_eq!(table_cos(&table, 0), 1.0);
    assert_eq!(table_cos(&table, -0x200), 0.0);
    assert_eq!(table_sin(&table, 0x800 + 5), table_sin(&table, 5));
    assert_eq!(angle_to_index(0.0), 0);
    assert_eq!(angle_to_index(std::f32::consts::FRAC_PI_2), 512);
    assert_eq!(follow_velocity_after_reset(), [0.0; 3]);
}

struct Stub {
    finished: Vec<u32>,
    tags: Vec<u32>,
    flags: u32,
    area: Option<u32>,
}

impl HookView for Stub {
    fn slot_finished(&self, slot: u32) -> bool {
        self.finished.contains(&slot)
    }
    fn npc(&self, tag: u32) -> Option<NpcView> {
        self.tags.contains(&tag).then_some(NpcView {
            model: 0,
            alive: true,
        })
    }
    fn player_area(&self) -> Option<u32> {
        self.area
    }
    fn mode_byte(&self) -> u8 {
        2
    }
    fn game_flags(&self) -> u32 {
        self.flags
    }
}

#[test]
fn level_views_are_built_from_the_hook_view() {
    let stub = Stub {
        finished: vec![9, 1],
        tags: vec![0x4F49_4C59, 0x5749_4E44, 0x4A42_5448],
        flags: 0x40,
        area: Some(0x4C),
    };
    let players = [
        PlayerRecord {
            alive: true,
            character: 7,
            state: 0x4C,
            flag_570: true,
            word_1568: 9,
        },
        PlayerRecord {
            alive: false,
            character: 8,
            state: 1,
            flag_570: false,
            word_1568: 0,
        },
    ];
    let l2 = level2_inputs(&stub, 0x40, -1, 12);
    assert!(l2.flag_40 && l2.mode_word == 0x40 && l2.ombh_count == 12 && l2.ambience_handle == -1);

    let l6 = level06_view(
        &stub,
        &players,
        &Level06Engine {
            slot4_above_one: true,
            ..Default::default()
        },
    );
    assert_eq!((l6.mode_id, l6.player_count), (2, 2));
    assert!(l6.slot9_finished && l6.slot4.above_one && !l6.slot4.finished && l6.local_player);
    assert_eq!(l6.states, vec![0x4C, 1]);
    assert_eq!(l6.words_1568, vec![9, 0]);
    assert_eq!(l6.player_flag_570, vec![true, false]);

    let lb = level0b_view(&stub, &players, &Level0bEngine::default());
    assert_eq!(lb.tags_present, vec![0x4F49_4C59]);
    assert!(lb.wind_present);
    assert_eq!(lb.local_state, Some(0x4C));
    assert_eq!(lb.players.len(), 2);

    let l11 = level11_view(
        &stub,
        &players,
        &Level11Engine {
            htbj_tick_zero: true,
            ..Default::default()
        },
    );
    assert_eq!(l11.htbj, Some(true));
    assert_eq!(l11.jani, None);
    assert!(l11.slot1.finished && !l11.slot1.above_one);
}
