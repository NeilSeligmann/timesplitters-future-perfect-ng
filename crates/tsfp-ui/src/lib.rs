//! ui: Rust ports of proven ui/menu roots (T1836 wave 3).
//!
//! Only `0022DF10` (`game_record_a0_has_bit`) is both proven against the original
//! (T1811, 60/60) and assigned to the ui/menu tree. See docs/t1836-rust-wave3.md.

#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod call_leaves;
pub mod flag_leaves;
pub mod hud;
pub mod hud_text;
pub mod hud_leaves;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
pub mod menu_leaves;
pub mod native_text;
#[path = "../../tsfp-engine-memory/src/strmem.rs"]
pub mod strmem;
pub mod text;
pub mod text_entry;
pub mod w4_ui;

/// Guest byte memory with 32-bit wrapping addresses (little endian).
pub trait UiMemory {
    fn read8(&self, address: u32) -> u8;
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
}

const BIT_WORD_A0: u32 = 0xa0;

/// 0022DF10 game_record_a0_has_bit: tests `bit` (shift count masked to 5 bits) of O+0xA0.
pub fn record_a0_has_bit<M: UiMemory>(m: &M, object: u32, bit: u32) -> u32 {
    u32::from(m.read32(object.wrapping_add(BIT_WORD_A0)) & 1u32.wrapping_shl(bit) != 0)
}
pub mod synth_leaves;
