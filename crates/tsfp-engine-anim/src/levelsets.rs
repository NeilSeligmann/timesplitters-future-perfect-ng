//! Level animation set tables, `0x1CA2F0` (T1668, rule 10 batch 7). Tier 1: `tests/levelsets_golden.rs` replays 400 seeded mount patterns against the
//! original under Unicorn (`0x62260` and `0x5B980` stubbed).
//!
//! The routine is a straight line of 103 steps: `slot = 0x62260(pak path)` (mount lookup, -1 when the pak is not mounted) and, when mounted,
//! `0x5B980("anim/data/ts3/", entries, count)` loads that pak's anim set table (24 byte entries, see [`crate::loaders`]). `entries` is the
//! virtual address of the table in the image (parse them with `tsfp_format_anim::propset`).

/// `(pak path, VA of the 24 byte entry table, entry count)` in the order the original tries them.
pub const ANIM_SET_PAKS: [(&str, u32, u32); 103] = [
    ("pak/anim/gun/minigun.pak", 0x47e758, 20),
    ("pak/stream/lv14/anim/lv14.pak", 0x47e938, 41),
    ("pak/stream/lv75/anim/lv75.pak", 0x47ed10, 4),
    ("pak/anim/gun/fshotgun.pak", 0x47ed70, 18),
    ("pak/anim/anim_z.pak", 0x47ef20, 21),
    ("pak/anim/gun/baseball.pak", 0x47f118, 18),
    ("pak/anim/anim_ts.pak", 0x47f2c8, 23),
    ("pak/anim/gun/mines.pak", 0x47f4f0, 34),
    ("pak/anim/gun/assaultr.pak", 0x47f820, 24),
    ("pak/anim/gun/assaults.pak", 0x47fa60, 20),
    ("pak/anim/gun/plasma.pak", 0x47fc40, 18),
    ("pak/anim/animcar.pak", 0x47fdf0, 27),
    ("pak/anim/anim_mg.pak", 0x480078, 2),
    ("pak/stream/lv56/anim/lv56.pak", 0x4800a8, 10),
    ("pak/stream/lv5/anim/lv5_1.pak", 0x480198, 15),
    ("pak/stream/lv5/anim/lv5_2.pak", 0x480300, 15),
    ("pak/stream/lv74/anim/lv74.pak", 0x480468, 1),
    ("pak/stream/lv5/anim/lv5_3.pak", 0x480480, 10),
    ("pak/stream/lv5/anim/lv5_4.pak", 0x480570, 5),
    ("pak/stream/lv18/anim/lv18_1.pak", 0x4805e8, 51),
    ("pak/anim/gun/electro.pak", 0x480ab0, 18),
    ("pak/stream/lv22/anim/lv22.pak", 0x480c60, 73),
    ("pak/stream/lv18/anim/lv18_2.pak", 0x481338, 34),
    ("pak/stream/lv18/anim/lv18_3.pak", 0x481668, 19),
    ("pak/anim/gun/flare.pak", 0x481830, 45),
    ("pak/anim/gun/frocketl.pak", 0x481c68, 26),
    ("pak/stream/lv46/anim/lv46.pak", 0x481ed8, 3),
    ("pak/stream/lv64/anim/lv64.pak", 0x481f20, 19),
    ("pak/anim/ts2_amb.pak", 0x4820e8, 25),
    ("pak/stream/lv22/anim/lv22_1.pak", 0x482340, 30),
    ("pak/stream/lv6/anim/lv6.pak", 0x482610, 70),
    ("pak/stream/lv22/anim/lv22_2.pak", 0x482ca0, 17),
    ("pak/stream/lv9/anim/lv9.pak", 0x482e38, 63),
    ("pak/stream/lv22/anim/lv22_3.pak", 0x483420, 28),
    ("pak/anim/gun/magnum.pak", 0x4836c0, 22),
    ("pak/anim/gun/rifle.pak", 0x4838d0, 22),
    ("pak/stream/lv18/anim/lv18.pak", 0x483ae0, 19),
    ("pak/anim/anim.pak", 0x483ca8, 76),
    ("pak/stream/lv63/anim/lv63.pak", 0x4843c8, 3),
    ("pak/stream/lv11/anim/lv11.pak", 0x484410, 33),
    ("pak/anim/gun/brick.pak", 0x484728, 18),
    ("pak/anim/anim_prt.pak", 0x4848d8, 70),
    ("pak/stream/lv81/anim/lv81.pak", 0x484f68, 20),
    ("pak/anim/gun/luger.pak", 0x485148, 51),
    ("pak/anim/gun/fpistol.pak", 0x485610, 45),
    ("pak/anim/gun/sbp500.pak", 0x485a48, 18),
    ("pak/anim/feanims.pak", 0x485bf8, 363),
    ("pak/stream/lv69/anim/lv69.pak", 0x487e00, 10),
    ("pak/anim/gun/machinep.pak", 0x487ef0, 55),
    ("pak/anim/arc_b.pak", 0x488418, 45),
    ("pak/stream/lv17/anim/lv17.pak", 0x488850, 42),
    ("pak/stream/lv11/anim/lv11_1.pak", 0x488c40, 37),
    ("pak/anim/animts3.pak", 0x488fb8, 109),
    ("pak/stream/lv11/anim/lv11_2.pak", 0x4899f0, 5),
    ("pak/stream/lv26/anim/lv26.pak", 0x489a68, 63),
    ("pak/stream/lv11/anim/lv11_3.pak", 0x48a050, 23),
    ("pak/stream/lv11/anim/lv11_4.pak", 0x48a278, 6),
    ("pak/anim/gun/harpoon.pak", 0x48a308, 16),
    ("pak/anim/gun/fsniper.pak", 0x48a488, 20),
    ("pak/anim/gun/rocketl.pak", 0x48a668, 18),
    ("pak/stream/lv71/anim/lv71.pak", 0x48a818, 1),
    ("pak/anim/gun/doublesg.pak", 0x48a830, 18),
    ("pak/anim/bpose.pak", 0x48a9e0, 1),
    ("pak/stream/lv1/anim/lv1.pak", 0x48a9f8, 58),
    ("pak/stream/lv4/anim/lv4.pak", 0x48af68, 20),
    ("pak/stream/lv80/anim/lv80.pak", 0x48b148, 11),
    ("pak/anim/anim_cov.pak", 0x48b250, 102),
    ("pak/stream/lv14/anim/lv14_1.pak", 0x48bbe0, 11),
    ("pak/stream/lv1/anim/lv1_1.pak", 0x48bce8, 7),
    ("pak/stream/lv14/anim/lv14_2.pak", 0x48bd90, 13),
    ("pak/stream/lv1/anim/lv1_2.pak", 0x48bec8, 30),
    ("pak/stream/lv14/anim/lv14_3.pak", 0x48c198, 1),
    ("pak/stream/lv1/anim/lv1_3.pak", 0x48c1b0, 16),
    ("pak/stream/lv77/anim/lv77.pak", 0x48c330, 2),
    ("pak/stream/lv1/anim/lv1_4.pak", 0x48c360, 7),
    ("pak/anim/gun/charger.pak", 0x48c408, 20),
    ("pak/stream/lv1/anim/lv1_5.pak", 0x48c5e8, 14),
    ("pak/stream/lv1/anim/lv1_6.pak", 0x48c738, 18),
    ("pak/anim/gun/uplink.pak", 0x48c8e8, 44),
    ("pak/stream/lv61/anim/lv61.pak", 0x48cd08, 1),
    ("pak/stream/lv4/anim/lv4_1.pak", 0x48cd20, 27),
    ("pak/anim/animgren.pak", 0x48cfa8, 10),
    ("pak/stream/lv4/anim/lv4_2.pak", 0x48d098, 15),
    ("pak/stream/lv4/anim/lv4_3.pak", 0x48d200, 39),
    ("pak/stream/lv17/anim/lv17_1.pak", 0x48d5a8, 30),
    ("pak/stream/lv4/anim/lv4_4.pak", 0x48d878, 10),
    ("pak/stream/lv17/anim/lv17_2.pak", 0x48d968, 20),
    ("pak/stream/lv4/anim/lv4_5.pak", 0x48db48, 24),
    ("pak/stream/lv17/anim/lv17_3.pak", 0x48dd88, 17),
    ("pak/anim/gun/ghost.pak", 0x48df20, 18),
    ("pak/stream/lv4/anim/lv4_6.pak", 0x48e0d0, 19),
    ("pak/stream/lv17/anim/lv17_4.pak", 0x48e298, 38),
    ("pak/anim/gun/pistol9.pak", 0x48e628, 51),
    ("pak/stream/lv76/anim/lv76.pak", 0x48eaf0, 2),
    ("pak/anim/gun/flame.pak", 0x48eb20, 18),
    ("pak/anim/gun/nemosmg.pak", 0x48ecd0, 58),
    ("pak/anim/animrb.pak", 0x48f240, 21),
    ("pak/anim/gun/dartgun.pak", 0x48f438, 16),
    ("pak/anim/gun/tnt.pak", 0x48f5b8, 16),
    ("pak/anim/gun/grenade.pak", 0x48f738, 16),
    ("pak/stream/lv60/anim/lv60.pak", 0x48f8b8, 1),
    ("pak/stream/lv2/anim/lv2.pak", 0x48f8d0, 110),
    ("pak/stream/lv5/anim/lv5.pak", 0x490320, 35),
];

/// The loads the original performs: `(entries VA, count)` of every pak for which `mounted(path)` is true, in order.
pub fn level_set_loads(mounted: impl Fn(&str) -> bool) -> Vec<(u32, u32)> {
    ANIM_SET_PAKS.iter().filter(|(path, _, _)| mounted(path)).map(|&(_, entries, count)| (entries, count)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete() {
        assert_eq!(ANIM_SET_PAKS.len(), 103);
        assert_eq!(ANIM_SET_PAKS.iter().map(|p| p.2).sum::<u32>(), 3062);
        assert_eq!(level_set_loads(|p| p == "pak/anim/anim.pak").len(), 1);
        assert!(level_set_loads(|_| false).is_empty());
    }
}
