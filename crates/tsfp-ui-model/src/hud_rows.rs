// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned HUD row allocation. Verification Tier 0: fabricated contract tests;
//! original per-frame reset and retained consumers attach at the live boundary.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerRows {
    pub ammo: [u8; 2],
    pub status_top: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HudRows {
    pub players: [PlayerRows; 4],
}

impl HudRows {
    /// Original 0x14B090 begins a new per-viewer HUD allocation cycle.
    pub fn begin(&mut self, player: usize) -> bool {
        let Some(rows) = self.players.get_mut(player) else {
            return false;
        };
        *rows = PlayerRows::default();
        true
    }

    pub fn ammo_index(&self, player: usize, side: usize) -> Option<u8> {
        self.players.get(player)?.ammo.get(side).copied()
    }

    /// The numeric drawer allocates one row. Advance is an original byte wrap,
    /// independent of the other side and of neighbouring HUD lifecycle fields.
    pub fn allocate_ammo(&mut self, player: usize, side: usize) -> Option<u8> {
        let cursor = self.players.get_mut(player)?.ammo.get_mut(side)?;
        let previous = *cursor;
        *cursor = cursor.wrapping_add(1);
        Some(previous)
    }

    pub fn set_ammo(&mut self, player: usize, side: usize, index: u8) -> bool {
        let Some(cursor) = self
            .players
            .get_mut(player)
            .and_then(|p| p.ammo.get_mut(side))
        else {
            return false;
        };
        *cursor = index;
        true
    }

    /// First row borrows the caller's layout origin once; later rows consume
    /// the owned column. The pointer of the retained caller is not stored.
    pub fn begin_status(&mut self, player: usize, origin: i32) -> Option<i32> {
        Some(
            *self
                .players
                .get_mut(player)?
                .status_top
                .get_or_insert(origin),
        )
    }

    pub fn status_top(&self, player: usize) -> Option<i32> {
        self.players.get(player)?.status_top
    }

    pub fn set_status(&mut self, player: usize, top: i32) -> bool {
        let Some(rows) = self.players.get_mut(player) else {
            return false;
        };
        rows.status_top = Some(top);
        true
    }

    pub fn advance_status(&mut self, player: usize, height: i32) -> Option<i32> {
        let top = self.players.get_mut(player)?.status_top.as_mut()?;
        *top = top.wrapping_add(height);
        Some(*top)
    }
}

/// Parent 0x14F0B0: after score rows, subtract the scaled gap and clamp the
/// effects origin against the viewport top plus margin (signed/wrapping).
pub fn effects_origin(top: i32, viewport_top: i32, margin: i32, scale: f32, gap: f32) -> i32 {
    let step = scale * gap;
    let step = if step.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&step) {
        step as i32
    } else {
        i32::MIN
    };
    let candidate = top.wrapping_sub(step);
    let minimum = viewport_top.wrapping_add(margin);
    if minimum <= candidate {
        candidate
    } else {
        minimum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_wraps_and_preserves_player_side_and_column_boundaries() {
        let mut rows = HudRows::default();
        assert!(rows.set_ammo(2, 0, 255));
        assert_eq!(rows.allocate_ammo(2, 0), Some(255));
        assert_eq!(rows.ammo_index(2, 0), Some(0));
        assert_eq!(rows.allocate_ammo(2, 1), Some(0));
        assert_eq!(rows.begin_status(2, 40), Some(40));
        assert_eq!(rows.advance_status(2, 22), Some(62));
        assert_eq!(rows.begin_status(2, 999), Some(62));
        assert!(rows.set_status(2, i32::MAX));
        assert_eq!(rows.advance_status(2, 1), Some(i32::MIN));
        let saved = rows.clone();
        assert_eq!(rows.allocate_ammo(4, 0), None);
        assert_eq!(rows.allocate_ammo(0, 2), None);
        assert_eq!(rows.begin_status(4, 0), None);
        assert!(!rows.begin(4));
        assert_eq!(rows, saved);
        assert!(rows.begin(2));
        assert_eq!(rows.players[2], PlayerRows::default());
    }

    #[test]
    fn parent_effect_origin_uses_signed_clamp_and_sse_truncation() {
        assert_eq!(effects_origin(100, 10, 20, 1.5, 9.0), 87);
        assert_eq!(effects_origin(35, 10, 20, 1.5, 9.0), 30);
        assert_eq!(effects_origin(0, i32::MAX, 1, f32::NAN, 1.0), i32::MIN);
        assert_eq!(effects_origin(10, 0, 0, -1.5, 9.0), 23);
    }
}
