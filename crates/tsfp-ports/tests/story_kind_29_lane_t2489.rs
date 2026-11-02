// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_ports::story_event_condition_lane::{
    ConditionEvaluator, ConditionRecord, PlayerIdVariant, evaluate,
};

struct Facts {
    tag: bool,
    flag: Option<u32>,
}
impl ConditionEvaluator for Facts {
    fn player_count(&self) -> usize {
        0
    }
    fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
        false
    }
    fn first_player_ext_c08_nonzero(&mut self) -> bool {
        false
    }
    fn slot_status(&self, _: u32) -> Option<u8> {
        None
    }
    fn tag_exists(&self, key: i32) -> bool {
        key == 0x47415330 && self.tag
    }
    fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
        None
    }
    fn scene_flag_0x75bf0c(&self) -> Option<u32> {
        self.flag
    }
}

#[test]
fn kind29_reads_tag_or_exact_scene_flag_and_declines_unknown() {
    let record = ConditionRecord {
        kind: 29,
        args: [0; 4],
    };
    assert_eq!(
        evaluate(
            record,
            &mut Facts {
                tag: true,
                flag: None
            }
        ),
        Some(true)
    );
    assert_eq!(
        evaluate(
            record,
            &mut Facts {
                tag: false,
                flag: Some(1)
            }
        ),
        Some(true)
    );
    assert_eq!(
        evaluate(
            record,
            &mut Facts {
                tag: false,
                flag: Some(2)
            }
        ),
        Some(false)
    );
    assert_eq!(
        evaluate(
            record,
            &mut Facts {
                tag: false,
                flag: None
            }
        ),
        None
    );
}
