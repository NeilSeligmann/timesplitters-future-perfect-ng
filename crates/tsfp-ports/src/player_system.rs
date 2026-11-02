// SPDX-License-Identifier: GPL-3.0-or-later
//! T2019 gameplay/player attachment boundary. Names INFERRED; guest memory owns all state.
//! Edge report: `docs/t-system-player.md` (Attachment points).
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// The current player record pointer `[0x7B0C7C]`, shared with weapons, HUD and save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerRecord(pub u32);
impl PlayerRecord {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self(m.read32(0x7b0c7c))
    }
    pub fn field(self, offset: u32) -> u32 {
        self.0.wrapping_add(offset)
    }
}

/// Per-player float stat table `[0x75CCC0]`: base + 4 * (player * 0x273 + stat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStats(pub u32);
impl PlayerStats {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self(m.read32(0x75ccc0))
    }
    pub fn entry(self, player: u32, stat: u32) -> u32 {
        self.0.wrapping_add(
            player
                .wrapping_mul(0x273)
                .wrapping_add(stat)
                .wrapping_mul(4),
        )
    }
}

/// Session, level and frame inputs read by player code, owned by other systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerContext {
    pub record: PlayerRecord,
    /// `[0x7B0C48]` controlled object (position words at +0xA8).
    pub controlled_object: u32,
    /// `[0x790950]` local player count, `[0x7A2958]` extra (remote) players.
    pub player_count: u32,
    pub extra_players: u32,
    /// `[0x79094C]` game state / level id (0x65 is a special level).
    pub level: u32,
    /// `0x7DE455` mode byte (0xA party), `0x7DE458` session flags.
    pub mode: u8,
    pub flags: u32,
    pub tick_divisor: i32,
    pub tick_scale_bits: u32,
}
impl PlayerContext {
    pub fn read<M: GuestMemory>(m: &M) -> Self {
        Self {
            record: PlayerRecord::read(m),
            controlled_object: m.read32(0x7b0c48),
            player_count: crate::state_wire::get_players(m),
            extra_players: crate::state_wire::get_extra_players(m),
            level: crate::state_wire::get_level(m),
            mode: crate::state_wire::get_mode(m),
            flags: crate::state_wire::get_flags(m),
            tick_divisor: m.read32(0x4e8cec) as i32,
            tick_scale_bits: m.read32(0x4e794c),
        }
    }
}

/// Per-function hybrid A/B choice over both player modules; disabled and unknown roots
/// have no side effects. GuestCall remains the explicit edge to object, input, session,
/// HUD, render, missions, AI, audio, weapons, inventory and save.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    enabled
        && (crate::player_look_gate::run(
            va,
            va == crate::player_look_gate::ROOTS[0] && crate::player_look_gate::selected(),
            m,
            guest,
            r,
        )
            || crate::camera::run(va, m, r)
            || crate::player_look::run(
                va,
                va == crate::player_look::ROOTS[0] && crate::player_look::selected(),
                m, guest, r,
            )
            || crate::objchar_player_writer::run(va, m, guest, r)
            // T2122: this selected player root writes ObjectPool/CharacterSet fields through leaf setters.
            || crate::c2r::objchar_wired18::run(va, m, guest, r)
            // T2122: the movement root writes CharacterSet body fields through leaf setters.
            || crate::c2r::objchar_wired20::run(va, m, guest, r)
            // T2122: move-input root writes a CharacterSet body field through the leaf setter.
            || crate::c2r::objchar_wired21::run(va, m, guest, r)
            // T2122: per-frame player root routes ObjectPool/CharacterSet words through leaf setters.
            || crate::c2r::objchar_wired22::run(va, m, guest, r)
            // T2122: locomotion selection writes ObjectPool/CharacterSet fields through leaf setters.
            || crate::c2r::objchar_wired23::run(va, m, guest, r)
            // T2122 closing continuation: hit-frame stamps route CharacterSet words through leaf setters.
            || crate::c2r::objchar_wired28::run(va, m, guest, r)
            // T3398: the opt-in D01C0 attachment wraps the retained scanner at its player-root seam.
            || crate::player_interaction_runtime::run(va, m, guest, r)
            || crate::player_interaction_runtime::run_candidate_filter(va, m, guest, r)
            || crate::c2r::player_wired::run(va, m, guest, r)
            // T2122: 0xC5A90 is a player-lane root whose stores go through ObjectPool/CharacterSet
            || crate::c2r::objchar_wired::run(va, m, guest, r)
            // T2126: weapon-lane roots 0xD8C80 0xD19F0 0xD84C0 0x25FB70 (stores through ObjectPool/CharacterSet)
            || crate::c2r::objchar_wired2::run(va, m, guest, r)
            || crate::c2r::leaf_wired::run(va, m, guest, r)
            || crate::c2r::gamestate_wired::run(va, m, guest, r)
            || crate::c2r::gameplay_player::run(va, m, guest, r)
            || tsfp_player_interaction_fast_path::dispatch_player_module(
                va,
                m,
                guest,
                r,
                crate::c2r::gameplay_player_2::run,
            )
            || crate::c2r::gameplay_player_3::run(va, m, guest, r)
            || crate::c2r::gameplay_player_stats::run(va, m, guest, r))
}

/// End-of-round stats/unlock update `0x23A040` (T2027): the original first-call callees it
/// reaches through `GuestCall`, with the neighbouring system that owns each. Names INFERRED.
pub const END_OF_ROUND_STATS: u32 = 0x23a040;
pub const END_OF_ROUND_STATS_EDGES: &[(u32, &str, &str)] = &[
    (0x3c9d90, "core", "_chkstk, allocates the 0x19A4-byte frame"),
    (0x3c848c, "core", "ftol"),
    (0x153b40, "core/rng", "rng_advance_lcg64"),
    (
        0x2382b0,
        "gameplay/player",
        "slot_below_active_count_for_mode",
    ),
    (0x239680, "gameplay/player", "stat_index_of_lowest_value"),
    (0x239710, "gameplay/player", "stat_index_of_highest_value"),
    (0x2397a0, "gameplay/player", "stat_average"),
    (
        0x1cd040,
        "gameplay/unlocks",
        "unlock_bitmask_of_13_entries_met",
    ),
    (0x069900, "save/profile", "profile_stats_record_ptr_by_slot"),
    (0x03d7b0, "data/text", "text_string_get_by_id_from_bank"),
    (0x1cdae0, "network/session", "net_effective_player_count"),
    (0x3533f0, "network/session", "net_is_session_active"),
    (
        0x356800,
        "network/session",
        "remap_player_index_via_session_table",
    ),
    (
        0x356940,
        "network/session",
        "count_used_record_124_when_online",
    ),
    (0x35de70, "network/live", "return_constant_0"),
    (0x35e960, "network/live", "is_live_mode_active"),
    (0x35eaf0, "network/live", "advance_session_key_counter_once"),
    (0x35ec20, "network/live", "format_session_key_string"),
    (0x35ecc0, "network/live", "fill_search_entry_from_record_18"),
];

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::collections::BTreeMap;
    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected boundary call");
        }
    }
    struct Session(bool, u32);
    impl GuestCall for Session {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            assert_eq!(va, 0x3533f0);
            self.1 += 1;
            r.eax = self.0 as u32;
        }
    }

    struct Calls;
    impl GuestCall for Calls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {}
    }

    fn registers() -> ExtRegisters {
        // All-zero integer and float bit patterns are valid register states.
        unsafe { core::mem::zeroed() }
    }
    #[test]
    fn end_of_round_edges_are_unique_and_route_through_the_switch() {
        let mut vas: std::vec::Vec<u32> = END_OF_ROUND_STATS_EDGES.iter().map(|e| e.0).collect();
        vas.sort_unstable();
        vas.dedup();
        assert_eq!(vas.len(), 19);
        let mut m = Memory::default();
        let mut r = registers();
        assert!(!run(
            END_OF_ROUND_STATS,
            false,
            &mut m,
            &mut NoCalls,
            &mut r
        ));
        assert!(m.0.is_empty());
    }

    #[test]
    fn disabled_and_unknown_leave_state_untouched() {
        let mut m = Memory::default();
        let mut r = registers();
        r.eax = 0x12345678;
        let mut c = NoCalls;
        assert!(!run(0x2382b0, false, &mut m, &mut c, &mut r));
        assert!(!run(0xffffffff, true, &mut m, &mut c, &mut r));
        assert_eq!(r.eax, 0x12345678);
        assert!(m.0.is_empty());
    }

    #[test]
    fn t2122_player_root_uses_object_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0x25a7f0,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired18::run(
            0x25a7f0,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        assert_eq!(routed_regs, direct_regs);
    }

    #[test]
    fn t2122_movement_root_uses_object_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0x258f50,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired20::run(
            0x258f50,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        // Zero memory drives the sweep maths to 0/0, so st7 is NaN in both runs and f64 `==`
        // can never hold. Compare the Debug rendering, which prints NaN as NaN.
        assert_eq!(
            std::format!("{routed_regs:?}"),
            std::format!("{direct_regs:?}")
        );
    }

    #[test]
    fn t2122_move_input_root_uses_object_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0xd2b80,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired21::run(
            0xd2b80,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        assert_eq!(routed_regs, direct_regs);
    }

    #[test]
    fn t2122_per_frame_root_uses_object_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0xd5eb0,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired22::run(
            0xd5eb0,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        assert_eq!(routed_regs, direct_regs);
    }

    #[test]
    fn t2122_locomotion_root_uses_object_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0x25c650,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired23::run(
            0x25c650,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        assert_eq!(routed_regs, direct_regs);
    }

    #[test]
    fn t2122_hit_frame_writer_uses_character_leaf_setter_translation() {
        let mut routed = Memory::default();
        let mut direct = Memory::default();
        let mut routed_regs = registers();
        let mut direct_regs = registers();
        let mut routed_calls = Calls;
        let mut direct_calls = Calls;

        assert!(run(
            0xe94e0,
            true,
            &mut routed,
            &mut routed_calls,
            &mut routed_regs,
        ));
        assert!(crate::c2r::objchar_wired28::run(
            0xe94e0,
            &mut direct,
            &mut direct_calls,
            &mut direct_regs,
        ));
        assert_eq!(routed.0, direct.0);
        assert_eq!(routed_regs, direct_regs);
    }

    #[test]
    fn context_and_tables_borrow_guest_state() {
        let mut m = Memory::default();
        m.write32(0x7b0c7c, 0x1000);
        m.write32(0x7b0c48, 0x2000);
        m.write32(0x790950, 2);
        m.write32(0x7a2958, 1);
        m.write32(0x79094c, 0x65);
        m.write8(0x7de455, 10);
        m.write32(0x7de458, 0x40);
        m.write32(0x4e8cec, 3);
        m.write32(0x4e794c, 0x3f800000);
        m.write32(0x75ccc0, 0x8000);
        let context = PlayerContext::read(&m);
        assert_eq!(context.record.field(0x1500), 0x2500);
        assert_eq!(
            (
                context.controlled_object,
                context.player_count,
                context.extra_players
            ),
            (0x2000, 2, 1)
        );
        assert_eq!(
            (context.level, context.mode, context.flags),
            (0x65, 10, 0x40)
        );
        assert_eq!(
            (context.tick_divisor, context.tick_scale_bits),
            (3, 0x3f800000)
        );
        assert_eq!(PlayerStats::read(&m).entry(1, 2), 0x8000 + 4 * 0x275);
    }
    #[test]
    fn slot_limit_follows_mode_level_and_session() {
        let mut m = Memory::default();
        let mut r = registers();
        m.write32(0x790950, 2);
        m.write32(0x7a2958, 1);
        // Offline, no session flag: limit is local + extra players (3).
        r.esi = 2;
        assert!(run(0x2382b0, true, &mut m, &mut NoCalls, &mut r));
        assert_eq!(r.eax, 1);
        r.esi = 3;
        assert!(run(0x2382b0, true, &mut m, &mut NoCalls, &mut r));
        assert_eq!(r.eax, 0);
        // Session flag with an active session lifts the limit to 0x10 via the network edge.
        m.write8(0x7de458, 0x40);
        let mut session = Session(true, 0);
        r.esi = 15;
        assert!(run(0x2382b0, true, &mut m, &mut session, &mut r));
        assert_eq!((r.eax, session.1), (1, 1));
        // Party mode caps slots below 2 before asking the session.
        m.write8(0x7de455, 10);
        r.esi = 2;
        assert!(run(0x2382b0, true, &mut m, &mut session, &mut r));
        assert_eq!((r.eax, session.1), (0, 1));
    }
}
