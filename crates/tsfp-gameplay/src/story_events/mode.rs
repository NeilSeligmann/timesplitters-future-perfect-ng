//! Mode predicate of the event and slot builders (T1841, decoded from `build/default.xbe`).
//!
//! `0x6B2B0(mask)` is the predicate the pool builder `0x235630` applies to `R+0xB4`. It tests
//! the mask against the running game configuration. `0x6B100(mask)` is the base predicate
//! (also used by the slot manager `0x227750` with mask 0x700 or 0xE00077F). Both are MEASURED
//! instruction by instruction; only the names of the configuration fields are INFERRED.

/// Mode id for which the Story event masks (0x700 family) are meant.
pub const MODE_STORY: u8 = 10;

/// Game configuration read by the predicates. Addresses are the original globals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeContext {
    /// `[0x7DE458]`: bit 0x40 and 0x8000000 select the descriptor path, 0x20000000 the bit-18 path.
    pub game_flags: u32,
    /// `[0x7DE455]`: game mode id 1 to 10 (10 = Story, the 0x700 masks).
    pub mode_id: u8,
    /// `[0x7DE457]`: sub-mode 0 to 2 (selects mask bit 0x100, 0x200 or 0x400 in mode 10).
    pub submode: u8,
    /// `[0x7DE49A]`: difficulty 0 to 2.
    pub difficulty: u8,
    /// `[0x790950]`: number of local players (mask bits 0x200000 = one, 0x400000 = two).
    pub players: u32,
    /// `[0x76B108]`: network-session flag (mask bit 31 fails when set).
    pub network: bool,
    /// Descriptor word `+0x14` of the table entry chosen by `[0x7DE470]` (`0x230CC0`), used when
    /// `game_flags & 0x8000040 != 0`.
    pub descriptor_word: u32,
    /// `[0x79094C]`: current level id (T1844: 0x38, 0x64 to 0x66 are special, see `levels`). 0 = none.
    pub level_id: u32,
}

impl Default for ModeContext {
    /// Story mode, sub-mode 0, difficulty 0, one player.
    fn default() -> Self {
        Self {
            game_flags: 0,
            mode_id: MODE_STORY,
            submode: 0,
            difficulty: 0,
            players: 1,
            network: false,
            descriptor_word: 0,
            level_id: 0,
        }
    }
}

impl ModeContext {
    pub fn story(difficulty: u8, submode: u8) -> Self {
        Self {
            difficulty,
            submode,
            ..Self::default()
        }
    }

    /// `0x6B100(mask)`.
    pub fn base_predicate(&self, mask: u32) -> bool {
        if self.game_flags & 0x40 != 0 || self.game_flags & 0x800_0000 != 0 {
            return self.descriptor_word & mask != 0;
        }
        if mask & 0x8000_0000 != 0 && self.network {
            return false;
        }
        if mask & 0x60_0000 != 0 {
            let one = mask & 0x20_0000 != 0 && self.players == 1;
            let two = mask & 0x40_0000 != 0 && self.players == 2;
            if !(one || two) {
                return false;
            }
        }
        // Jump table `0x6B284`, index = mode id - 1 (modes 1 to 10 only).
        match self.mode_id {
            1 => mask & 0x02 != 0,
            2 => mask & 0x04 != 0,
            3 => mask & 0x01 != 0,
            4 | 9 => mask & 0x08 != 0,
            5 => mask & 0x40 != 0,
            6 => mask & 0x20 != 0,
            7 => mask & 0x10 != 0,
            10 => self.story_predicate(mask),
            _ => false,
        }
    }

    fn story_predicate(&self, mask: u32) -> bool {
        let difficulty_bits = mask & 0x3800;
        if difficulty_bits != 0 {
            let selected = [0x800, 0x1000, 0x2000]
                .get(self.difficulty as usize)
                .copied()
                .unwrap_or(0);
            if selected & difficulty_bits == 0 {
                return false;
            }
        }
        match self.submode {
            0 => mask & 0x100 != 0,
            1 => mask & 0x200 != 0,
            2 => mask & 0x400 != 0,
            _ => false,
        }
    }

    /// `0x6B2B0(mask)`: the build-time predicate for event definitions.
    pub fn build_predicate(&self, mask: u32) -> bool {
        if self.game_flags & 0x2000_0000 != 0 {
            return (mask >> 18) & 1 != 0;
        }
        self.base_predicate(mask)
    }
}
