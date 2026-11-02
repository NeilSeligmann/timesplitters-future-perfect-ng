//! Weapon-row subrecord lookup from original `0x1B99F0`.

pub const WEAPON_ROW_COUNT: i32 = 70;
pub const AMMO_SUBRECORD_COUNT: i32 = 66;
pub const WEAPON_ROW_STRIDE: u32 = 0x3c;
pub const AMMO_SUBRECORD_OFFSET: u32 = 0x1c;

/// Resolve a signed slot's mapped row id and return that row's `+0x1c` field.
/// `row_field` is the already-read field at the typed boundary.
pub fn resolve_row_field(slot: i32, row_id: i32, row_field: Option<u32>) -> u32 {
    if !(0..WEAPON_ROW_COUNT).contains(&slot) || !(0..AMMO_SUBRECORD_COUNT).contains(&row_id) {
        return 0;
    }
    row_field.unwrap_or(0)
}
