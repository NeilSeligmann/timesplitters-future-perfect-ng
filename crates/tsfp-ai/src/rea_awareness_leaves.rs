// SPDX-License-Identifier: GPL-3.0-or-later
//! Small awareness and route leaves of the REA AI (T2425).
//!
//! Four original functions, each read with REA (Ghidra on `tsfp-analysis-v4.elf`, sha256
//! `e99faaeb...f866f`, MEASURED static):
//!
//! * `0x349E20(value)` sets the alert value of the four group records,
//! * `0xA0640(index)` registers the level spawn entries of one group index,
//! * `0x15D470(actor)` refreshes the actor nav node from the link query,
//! * `0x3797B0(actor, out)` builds the actor's unit view direction.

/// Group alert value cells (`+0x3C` of the four stride-`0x68` group records).
pub const GROUP_ALERT_VALUE_CELLS: [u32; 4] = [0x77883c, 0x7788a4, 0x77890c, 0x778974];

/// `0x349E20`: write one value to every group alert value cell.
pub fn set_group_alert_value(mut write_cell: impl FnMut(u32, u32), value: u32) {
    for cell in GROUP_ALERT_VALUE_CELLS {
        write_cell(cell, value);
    }
}

/// Spawn entry stride of the level record and the field offsets used by `0xA0640`.
pub const SPAWN_ENTRY_STRIDE: u32 = 0x34;
const ENTRY_KIND: u32 = 0x24;
const ENTRY_GROUP_KEY: u32 = 0x28;
/// Entry kind that registers (`1`).
pub const SPAWN_KIND_REGISTERED: u32 = 1;

/// World of `0xA0640`: the level spawn record and its callees.
pub trait SpawnRegisterWorld {
    /// `0x6A850(level)`: level spawn record, 0 when the level has none.
    fn level_record(&mut self) -> u32;
    /// Record `+0xBC` entry table and `+0xC0` entry count (re-read every step).
    fn entry_table(&self, record: u32) -> u32;
    fn entry_count(&self, record: u32) -> i32;
    fn read32(&self, address: u32) -> u32;
    /// Pointer at `0x72F5EC`: per-entry registered counters.
    fn counters(&self) -> u32;
    /// `0x25FA10(key)`: group index of a group key.
    fn group_index_of(&mut self, key: u32) -> u32;
    /// `0xA05C0(entry)`: register one entry.
    fn register_entry(&mut self, entry: u32);
}

/// `0xA0640(index)`: register every kind-1 entry whose counter is still below 1 and whose
/// group key resolves to `index`. The count is read once for the entry guard and again
/// after every entry, as in the original.
pub fn register_spawn_entries<W: SpawnRegisterWorld>(world: &mut W, index: u32) {
    let record = world.level_record();
    if record == 0 {
        return;
    }
    let table = world.entry_table(record);
    if world.entry_count(record) <= 0 {
        return;
    }
    let counters = world.counters();
    let mut slot = 0u32;
    loop {
        let entry = table.wrapping_add(slot.wrapping_mul(SPAWN_ENTRY_STRIDE));
        if world.read32(entry.wrapping_add(ENTRY_KIND)) == SPAWN_KIND_REGISTERED
            && (world.read32(counters.wrapping_add(slot.wrapping_mul(4))) as i32) < 1
        {
            let key = world.read32(entry.wrapping_add(ENTRY_GROUP_KEY));
            if world.group_index_of(key) == index {
                world.register_entry(entry);
            }
        }
        slot = slot.wrapping_add(1);
        if (slot as i32) >= world.entry_count(record) {
            return;
        }
    }
}

/// `0x15D470(actor)`: ask the link query to refresh the nav node (`0x159E50(actor, 0, 0)`).
pub fn route_nav_update(mut link_query: impl FnMut(u32, u32, u32), actor: u32) {
    link_query(actor, 0, 0);
}

/// Constants of `0x3797B0` (`0x475C78`, `0x475D0C`, `0x475CE0`, `0x475CD4`, `0x4765F4`).
pub const ONE: f32 = 1.0;
pub const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3c8efa35);
pub const RADIANS_TO_TABLE: f32 = f32::from_bits(0x43a2f983);
pub const ROUND_HALF: f32 = 0.5;
pub const MIN_LENGTH_SQUARED: f32 = f32::from_bits(0x33d6bf95);
/// Sine table `0x7A38A0`: 2048 floats, a quarter turn is `0x200` entries.
pub const SINE_TABLE_MASK: i32 = 0x7ff;
pub const QUARTER_TURN: i32 = 0x200;

/// Aim angles of the actor's view source (`ext+0xAA0`): pitch at `+0x20`, yaw offset at `+0x24`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewAngles {
    pub pitch: f32,
    pub yaw_offset: f32,
}

fn table_angle(angle: f32) -> i32 {
    let scaled = angle * DEGREES_TO_RADIANS * RADIANS_TO_TABLE + ROUND_HALF;
    // `cvttss2si` returns the "integer indefinite" value for NaN and out of range inputs.
    if scaled.is_nan() || scaled >= 2147483648.0 || scaled < -2147483648.0 {
        i32::MIN
    } else {
        scaled as i32
    }
}

/// `0x3797B0`: unit view direction. `yaw` is the object heading (`+0x70`), `view` the source
/// angles or `None` when the extension has no source (forward `(0, 0, 1)`). `table` reads a
/// raw sine table word. The original's `rsqrtss` is modelled as the exact `1 / sqrt` (the same
/// choice as the lifted C, INFERRED for hardware).
pub fn view_direction(yaw: f32, view: Option<ViewAngles>, table: impl Fn(i32) -> u32) -> [u32; 3] {
    let mut out = match view {
        None => [0.0f32.to_bits(), 0.0f32.to_bits(), ONE.to_bits()],
        Some(view) => {
            let heading = table_angle(yaw + view.yaw_offset);
            let pitch = table_angle(view.pitch);
            let sine = |index: i32| f32::from_bits(table(index & SINE_TABLE_MASK));
            let x = sine(heading) * sine(pitch.wrapping_add(QUARTER_TURN));
            let y = table(pitch & SINE_TABLE_MASK);
            let z =
                sine(heading.wrapping_add(QUARTER_TURN)) * sine(pitch.wrapping_add(QUARTER_TURN));
            [x.to_bits(), y, z.to_bits()]
        }
    };
    let [x, y, z] = out.map(f32::from_bits);
    let length_squared = x * x + y * y + z * z;
    if length_squared > MIN_LENGTH_SQUARED {
        let scale =
            1.0 / (crate::perception_geometry::sqrt_f64(f64::from(length_squared)) as f32) * ONE;
        out = [
            (x * scale).to_bits(),
            (y * scale).to_bits(),
            (z * scale).to_bits(),
        ];
    }
    out
}
