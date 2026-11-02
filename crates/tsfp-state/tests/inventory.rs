// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::inventory::*;

/// Flat image of the low 8 MiB; the extension block lives at 0x200000 (pointer at EXT_POINTER).
struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v;
    }
}
const EXT: u32 = 0x20_0000;
fn image() -> Image {
    let mut m = Image((0..0x80_0000usize).map(|i| (i * 7 + 3) as u8).collect());
    m.write32(EXT_POINTER, EXT);
    // Synthetic table: every 3rd entry is an x1 head, then a left companion, then an x2.
    for i in 0..ENTRY_COUNT as u32 {
        let flags = match i % 3 {
            0 => 0,
            1 => FLAG_LEFT,
            _ => FLAG_X2,
        };
        m.write32(ENTRY_TABLE + i * ENTRY_STRIDE, flags);
    }
    m
}

#[test]
fn entries_round_trip_and_untouched_bytes() {
    let mut m = image();
    let before = m.0.clone();
    let t = WeaponEntries::load(&m);
    t.store(&mut m);
    assert_eq!(m.0, before);
    assert_eq!(t.diff(&m), None);
    assert_eq!(WeaponEntries::load(&m), t);
}

#[test]
fn group_numbering_matches_rule_and_writes_through() {
    let mut m = image();
    let mut t = WeaponEntries::load(&m);
    let groups = t.assign_groups(&mut m);
    assert_eq!(groups, 23);
    assert_eq!(
        (
            t.entries[0].group(),
            t.entries[1].group(),
            t.entries[2].group()
        ),
        (0, 0, 0)
    );
    assert_eq!(t.entries[3].group(), 1);
    assert_eq!(t.diff(&m), None);
    assert_eq!(m.read32(ENTRY_TABLE + 5 * ENTRY_STRIDE + 0x38), 1);
    assert_eq!(m.read32(MAX_GROUP_CELL), 23);
}

#[test]
fn entry_drift_is_found_per_word() {
    let mut m = image();
    let t = WeaponEntries::load(&m);
    m.write8(ENTRY_TABLE + 7 * ENTRY_STRIDE + 8, 0x5a);
    assert_eq!(t.diff(&m), Some((7, 2)));
}

#[test]
fn inventory_round_trip_and_untouched_bytes() {
    let mut m = image();
    let before = m.0.clone();
    let base = Inventory::slot_base(&m, 0).unwrap();
    let inv = Inventory::load(&m, base);
    inv.store(&mut m);
    assert_eq!(m.0, before);
    assert_eq!(inv.diff(&m), None);
    assert_eq!(Inventory::slot_base(&m, 2), Some(EXT + 2 * EXT_STRIDE));
    m.write32(EXT_POINTER, 0);
    assert_eq!(Inventory::slot_base(&m, 0), None);
}

#[test]
fn inventory_write_through_and_drift_per_field() {
    let mut m = image();
    let mut inv = Inventory::load(&m, EXT);
    inv.set_owned(&mut m, 5, 1);
    inv.set_equipped(&mut m, 6, 1);
    inv.set_ammo(&mut m, 43, 0xbeef);
    inv.set_current(&mut m, 9);
    inv.set_previous(&mut m, 8);
    inv.set_throwable(&mut m, 0x2c);
    inv.set_flags(&mut m, 0x100);
    assert_eq!(inv.diff(&m), None);
    assert_eq!(m.read8(EXT + EXT_OWNED + 5), 1);
    assert_eq!(m.read16(EXT + EXT_AMMO + 86), 0xbeef);
    assert_eq!(Inventory::load(&m, EXT), inv);
    let cells: [(u32, &str); 7] = [
        (EXT_FLAGS, "flags"),
        (EXT_CURRENT, "current"),
        (EXT_PREVIOUS, "previous"),
        (EXT_THROWABLE, "throwable"),
        (EXT_OWNED + 3, "owned"),
        (EXT_EQUIPPED + 3, "equipped"),
        (EXT_AMMO + 10, "ammo"),
    ];
    for (off, name) in cells {
        let mut d = Image(m.0.clone());
        let v = d.read8(EXT + off);
        d.write8(EXT + off, v ^ 0x40);
        assert_eq!(inv.diff(&d), Some(name), "{name}");
    }
}

#[test]
fn add_ammo_clamps_and_never_lowers() {
    let mut m = image();
    let mut inv = Inventory::load(&m, EXT);
    inv.set_ammo(&mut m, 1, 90);
    assert_eq!(inv.add_ammo(&mut m, 1, 20, 100), 10);
    assert_eq!(inv.ammo[1], 100);
    // Already above the cap (cap lowered): reserve is left alone.
    assert_eq!(inv.add_ammo(&mut m, 1, 5, 50), 0);
    assert_eq!(inv.ammo[1], 100);
    assert_eq!(inv.diff(&m), None);
}

#[test]
fn add_ammo_property_grid_preserves_reserve_and_guest_bytes() {
    let mut m = image();
    let mut inv = Inventory::load(&m, EXT);
    let starting = [0, 1, 49, 255, 65535];
    let additions = [0, 1, 17, 255, 65535];
    let caps = [0, 1, 100, 65535];
    for ammo_type in 0..AMMO_TYPES {
        for old in starting {
            for amount in additions {
                for max in caps {
                    inv.set_ammo(&mut m, ammo_type, old);
                    let expected = (u32::from(old) + u32::from(amount))
                        .min(u32::from(max))
                        .max(u32::from(old)) as u16;
                    assert_eq!(inv.add_ammo(&mut m, ammo_type, amount, max), expected - old);
                    assert_eq!(inv.ammo[ammo_type], expected);
                    assert_eq!(m.read16(EXT + EXT_AMMO + 2 * ammo_type as u32), expected);
                    assert_eq!(inv.diff(&m), None);
                }
            }
        }
    }
}

#[test]
fn grant_x1_owns_companion_x2_owns_group() {
    let mut m = image();
    let mut t = WeaponEntries::load(&m);
    t.assign_groups(&mut m);
    let mut inv = Inventory::load(&m, EXT);
    for i in 0..ENTRY_COUNT {
        inv.set_owned(&mut m, i, 0);
    }
    inv.set_flags(&mut m, 0);
    assert_eq!(inv.grant(&mut m, &t, 0), 2);
    assert!(inv.owns(0) && inv.owns(1) && !inv.owns(2));
    assert_eq!(inv.flags & FLAG_PICKUP, FLAG_PICKUP);
    assert_eq!(inv.grant(&mut m, &t, 2), 1);
    assert!(inv.owns(2) && !inv.owns(3));
    assert_eq!(inv.diff(&m), None);
}
