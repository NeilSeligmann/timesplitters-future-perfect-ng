//! Animation PAK list and anim set loading (T1668): `0x1C8540` and `0x5B980` (with its wrappers such as `0x1C9610`).
//!
//! * [`pak_list`] = the pak list of `0x1C8540` for a level (`[0x79094C]`), mode byte (`[0x7DE455]`, 10 = the
//!   full list of the level), flags (`[0x7DE458]`, bit `0x40`) and kind (`[0x7DE470]`): the `0x5A130` (open PAK)
//!   calls in order. MEASURED against the original under Unicorn (4160 combinations, `tools/t1668_loader_gold.py`,
//!   `tests/loader_golden.rs`). Not modelled: the first section driven by the level list `0x6BD30` (stubbed as null)
//!   and the final error report (level in a fixed set -> `0x3C85D4`/`0x5A130` of a message), level `0x66` returns at once.
//! * [`AnimSlots::load_set`] = the loop of `0x5B980(base, entries, count)`: per entry the clip `base + name + ".raw"`
//!   is loaded (`0x57F30`, which hands out a slot), `slot_of_id[id] = slot` (`0x7E03C0`) and the entry's two policy
//!   words are stored per slot (`0x7DE660 + 8 * slot`). INFERRED from the disassembly (the clip loader is the caller's closure).
//!   `0x1C9610` is the wrapper `0x5B980("anim/data/ts3/", table 0x48C198, 1)`, 136 such wrappers exist (T1875).
use std::collections::BTreeMap;
use tsfp_format_anim::propset::AnimSetEntry;

const PAK_DIR: &str = "pak/anim/";
// Level sets read from the original's jump tables (levels are the story level numbers, 100 = `0x64` frontend).
const PARTICLE: &[i32] = &[2, 4, 6, 9, 11, 14, 18, 22, 26, 81, 100];
const COVER: &[i32] = &[2, 4, 5, 6, 9, 11, 14, 18, 22, 26, 72, 73, 100];
const GRENADE: &[i32] = &[2, 4, 5, 6, 9, 14, 18, 22, 26, 100];
const CAR: &[i32] = &[4, 5, 18, 53, 100];
const ZOMBIE_Z: &[i32] = &[1, 17, 22, 56, 74, 100];
const RB: &[i32] = &[5, 9, 14, 72, 81, 100];
const TS_ALWAYS: &[i32] = &[2, 26, 100];
const STREAMED: &[i32] = &[
    1, 2, 4, 5, 6, 9, 11, 14, 17, 18, 22, 26, 46, 56, 60, 61, 63, 64, 69, 71, 74, 75, 76, 77, 80, 81,
];
/// Frontend level: every list entry is opened for it, and it has `anim_mg` even in the full list.
const FRONTEND: i32 = 100;
const FRONT_END_MENU: i32 = 101;
/// Level `0x66` returns without opening anything.
const NONE: i32 = 102;
/// The mode byte value that selects the level's full list.
pub const FULL_LIST_MODE: u8 = 10;

fn pak(name: &str) -> String {
    format!("{PAK_DIR}{name}.pak")
}

/// PAK files opened by `0x1C8540` in order.
pub fn pak_list(level: i32, mode: u8, flags: u32, kind: u32) -> Vec<String> {
    if level == NONE {
        return Vec::new();
    }
    let full = mode == FULL_LIST_MODE;
    let flagged = flags & 0x40 != 0;
    let mut list = Vec::new();
    if level == FRONT_END_MENU {
        list.push(pak("feanims"));
    }
    list.push(pak("anim"));
    list.push(pak("animts3"));
    if full {
        for (set, name) in [(PARTICLE, "anim_prt"), (COVER, "anim_cov"), (GRENADE, "animgren"), (CAR, "animcar")] {
            if set.contains(&level) {
                list.push(pak(name));
            }
        }
        if ZOMBIE_Z.contains(&level) {
            list.push(pak("anim_z"));
        }
        if flagged && kind <= 2 {
            list.push(pak("anim_z"));
        }
    } else if level == 0x35 || level == FRONTEND {
        list.push(pak("animcar"));
    }
    if !full || level == FRONTEND {
        list.push(pak("anim_mg"));
        if !full {
            list.push(pak("arc_b"));
        }
    }
    if full && RB.contains(&level) {
        list.push(pak("animrb"));
    }
    if flagged && kind == 0xF {
        list.push(pak("animrb"));
    }
    if flagged && matches!(kind, 0xC | 0xD | 0xE | 0x13) {
        list.push(pak("anim_ts"));
    }
    if full && TS_ALWAYS.contains(&level) {
        list.push(pak("anim_ts"));
    }
    if STREAMED.contains(&level) {
        list.push(format!("pak/stream/lv{level}/anim/lv{level}.pak"));
    }
    list
}

/// Clip slot bookkeeping of `0x5B980`: `slot_of_id` is `0x7E03C0`, `policy` is `0x7DE660`.
#[derive(Debug, Default)]
pub struct AnimSlots {
    pub slot_of_id: BTreeMap<u16, i16>,
    pub policy: BTreeMap<i16, [u32; 2]>,
}

impl AnimSlots {
    /// `0x5B980(base, entries, count)`: `load_clip(path)` is `0x57F30` and returns the clip slot (the id's later
    /// entries replace earlier ones, as in the original).
    pub fn load_set(&mut self, base: &str, entries: &[AnimSetEntry], mut load_clip: impl FnMut(&str) -> i16) {
        for entry in entries {
            let slot = load_clip(&format!("{base}{}.raw", entry.name));
            self.slot_of_id.insert(entry.id, slot);
            self.policy.insert(slot, entry.policy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_load_maps_ids_to_slots_and_policies() {
        let entry = |name: &str, id| AnimSetEntry { name: name.into(), policy: [id as u32, 7], id };
        let mut slots = AnimSlots::default();
        let mut seen = Vec::new();
        slots.load_set("anim/data/ts3/", &[entry("a", 5), entry("b", 6)], |path| {
            seen.push(path.to_string());
            seen.len() as i16
        });
        assert_eq!(seen, ["anim/data/ts3/a.raw", "anim/data/ts3/b.raw"]);
        assert_eq!(slots.slot_of_id[&6], 2);
        assert_eq!(slots.policy[&1], [5, 7]);
    }
}
