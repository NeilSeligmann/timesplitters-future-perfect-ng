// SPDX-License-Identifier: GPL-3.0-or-later
//! Private allocator-test recorder for the out-of-memory callback.
use tsfp_engine_memory::w4m_arena::ArenaHooks;

#[derive(Default)]
pub struct Hooks(pub Vec<u32>);
impl ArenaHooks for Hooks {
    fn out_of_memory(&mut self, argument: u32) {
        self.0.push(argument);
    }
}
