// SPDX-License-Identifier: GPL-3.0-or-later
//! Small weapon/player AI table view used by the yaw controller.

use crate::rea_character_leaves::TableView;

const ACTOR_RECORD_INDEX: u32 = 0xd8;
const TURN_RATE_VALUE: u32 = 0x18;
const TURN_RATE_TABLE: u32 = 0x007e_3954;

/// `0x59D50(actor)`: return the selected actor record's turn-rate scalar.
///
/// The original loads `actor+4`, reads that record's index at `+0xd8`, follows
/// the pointer table at `0x7E3954`, then loads the row's float at `+0x18` into
/// x87 ST0. Valid actor, record, index, and table pointers are source preconditions.
pub fn actor_turn_rate_bits<M: TableView>(memory: &M, actor: u32) -> u32 {
    let record = memory.read32(actor.wrapping_add(4));
    let index = memory.read32(record.wrapping_add(ACTOR_RECORD_INDEX));
    let table = memory.read32(TURN_RATE_TABLE);
    let row = memory.read32(table.wrapping_add(index.wrapping_mul(4)));
    memory.read32(row.wrapping_add(TURN_RATE_VALUE))
}
