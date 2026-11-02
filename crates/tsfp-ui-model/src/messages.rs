//! T2191 UI-8: the per-player HUD message ring as typed, readable Rust (`original` preset).
//!
//! Same behaviour as the Tier 1 guest-memory port `tsfp_ui::hud::messages` (0x12D6B0, 0x12D760,
//! 0x12D810 and the fade rule of 0x12D8E0), over owned state: no guest addresses, no retail
//! constants. Each of four players owns 16 entries filled round robin. Kind 0 means unused. An
//! entry is opaque until `fade_start` seconds of age and fades linearly to 0 at `end`. Timing
//! constants are supplied at run time from the original tables. Contract:
//! docs/t2191-ingame-ui.md, section "Message ring model".

/// Players with a ring.
pub const PLAYERS: usize = 4;
/// Entries per ring.
pub const RING_LEN: usize = 16;
/// Text capacity of an entry in bytes (the original copies with `strncpy(dst, src, 0x140)`).
pub const TEXT_BYTES: usize = 0x140;

/// Plain notice kind (0x12D6B0).
pub const KIND_NOTICE: u32 = 0x0a;
/// Notice variant kind (0x12D760).
pub const KIND_NOTICE_C: u32 = 0x0c;
/// Kinds that survive "clear everything" (selector -1).
pub const KEPT_BY_CLEAR_ALL: [u32; 3] = [4, 5, 0x0d];

/// Run-time constants from the original tables (never committed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageTuning {
    /// Age at which the fade starts, also the eviction age (0x475D10).
    pub fade_start: f32,
    /// Age at which the entry is gone (0x475D00).
    pub end: f32,
    /// Full opacity (0x475C78).
    pub one: f32,
}

/// One ring entry. Fields the producers never write (original `+0x15C..+0x164`) are not modelled.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    /// 0 when unused, else the producer's kind.
    pub kind: u32,
    /// Text bytes up to the terminator, at most [`TEXT_BYTES`] (Latin-1).
    pub text: Vec<u8>,
    /// Age in seconds; starts at `-delay` so a delayed notice waits before it counts.
    pub age: f32,
    pub fade_start: f32,
    pub end: f32,
    pub evict_after: f32,
    pub weight: u32,
    /// Opaque producer value, carried through for the drawer.
    pub opaque: u32,
}

impl Message {
    pub fn is_used(&self) -> bool {
        self.kind != 0
    }

    /// Opacity from the age: `one` until `fade_start`, then linear down to 0 at `end`. Not
    /// clamped, as in 0x12D8E0 (the per-frame update removes the entry at `end`).
    pub fn alpha(&self, one: f32) -> f32 {
        fade_alpha(self.age, self.fade_start, self.end, one)
    }

    /// Text widened to chars.
    pub fn text_string(&self) -> String {
        self.text.iter().map(|byte| char::from(*byte)).collect()
    }
}

/// The fade rule of 0x12D8E0. Anything but `age > fade_start` (NaN included) keeps the original
/// unordered compare: a NaN age is fully opaque.
pub fn fade_alpha(age: f32, fade_start: f32, end: f32, one: f32) -> f32 {
    if age.partial_cmp(&fade_start) != Some(std::cmp::Ordering::Greater) {
        one
    } else {
        one - (age - fade_start) / (end - fade_start)
    }
}

/// One player's ring.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageRing {
    /// Next entry to overwrite, always below [`RING_LEN`].
    pub cursor: usize,
    pub entries: [Message; RING_LEN],
}

impl MessageRing {
    /// Overwrite the entry at the cursor and advance it (shared body of 0x12D6B0 and 0x12D760).
    pub fn push(
        &mut self,
        kind: u32,
        text: &[u8],
        opaque: u32,
        delay: f32,
        tuning: &MessageTuning,
    ) {
        let slot = self.cursor;
        self.cursor = (slot + 1) % RING_LEN;
        let length = text
            .iter()
            .take(TEXT_BYTES)
            .take_while(|byte| **byte != 0)
            .count();
        self.entries[slot] = Message {
            kind,
            text: text[..length].to_vec(),
            // 0 - delay, not -delay: a zero delay gives +0.0 as the original's subss does
            age: 0.0 - delay,
            fade_start: tuning.fade_start,
            end: tuning.end,
            evict_after: tuning.fade_start,
            weight: 1,
            opaque,
        };
    }

    /// 0x12D810: mark unused every entry of `kind`; `kind == -1` clears all but 4, 5 and 0xD.
    pub fn clear_kind(&mut self, kind: i32) {
        for entry in &mut self.entries {
            let matches = entry.kind as i32 == kind
                || (kind == -1 && !KEPT_BY_CLEAR_ALL.contains(&entry.kind));
            if matches {
                entry.kind = 0;
            }
        }
    }
}

/// All players' rings. Calls naming a player outside 0..4 are ignored, as in the original.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudMessages {
    pub players: [MessageRing; PLAYERS],
}

impl HudMessages {
    fn ring(&mut self, player: i32) -> Option<&mut MessageRing> {
        usize::try_from(player)
            .ok()
            .and_then(|index| self.players.get_mut(index))
    }

    /// 0x12D6B0: push a plain notice.
    pub fn push_notice(
        &mut self,
        player: i32,
        text: &[u8],
        opaque: u32,
        delay: f32,
        tuning: &MessageTuning,
    ) {
        if let Some(ring) = self.ring(player) {
            ring.push(KIND_NOTICE, text, opaque, delay, tuning);
        }
    }

    /// 0x12D760: push the kind 0xC variant.
    pub fn push_notice_c(
        &mut self,
        player: i32,
        text: &[u8],
        opaque: u32,
        delay: f32,
        tuning: &MessageTuning,
    ) {
        if let Some(ring) = self.ring(player) {
            ring.push(KIND_NOTICE_C, text, opaque, delay, tuning);
        }
    }

    /// 0x12D810 for one player.
    pub fn clear_kind(&mut self, player: i32, kind: i32) {
        if let Some(ring) = self.ring(player) {
            ring.clear_kind(kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FABRICATED stand-in timing.
    const TUNING: MessageTuning = MessageTuning {
        fade_start: 2.0,
        end: 3.0,
        one: 1.0,
    };

    #[test]
    fn ring_wraps_and_truncates_text() {
        let mut hud = HudMessages::default();
        for index in 0..17u32 {
            hud.push_notice(1, b"hello\0junk", index, 0.0, &TUNING);
        }
        let ring = &hud.players[1];
        assert_eq!(ring.cursor, 1);
        assert_eq!(ring.entries[0].opaque, 16);
        assert_eq!(ring.entries[0].text, b"hello");
        assert_eq!(ring.entries[0].age.to_bits(), 0.0f32.to_bits());
        assert_eq!(hud.players[0], MessageRing::default());
        hud.push_notice(4, b"x", 0, 0.0, &TUNING);
        hud.push_notice_c(-1, b"x", 0, 0.0, &TUNING);
        assert_eq!(hud.players[1].cursor, 1);
        let long = [b'a'; TEXT_BYTES + 8];
        hud.push_notice_c(2, &long, 0, 1.5, &TUNING);
        assert_eq!(hud.players[2].entries[0].text.len(), TEXT_BYTES);
        assert_eq!(hud.players[2].entries[0].kind, KIND_NOTICE_C);
        assert_eq!(hud.players[2].entries[0].age, -1.5);
    }

    #[test]
    fn clear_all_keeps_sticky_kinds() {
        let mut ring = MessageRing::default();
        for (slot, kind) in [4, 5, 0x0d, 0x0a, 0x0c, 7].into_iter().enumerate() {
            ring.entries[slot].kind = kind;
        }
        ring.clear_kind(-1);
        let kinds: Vec<u32> = ring.entries[..6].iter().map(|entry| entry.kind).collect();
        assert_eq!(kinds, [4, 5, 0x0d, 0, 0, 0]);
        ring.clear_kind(5);
        assert_eq!(ring.entries[1].kind, 0);
        assert_eq!(ring.entries[0].kind, 4);
    }

    #[test]
    fn alpha_fades_linearly() {
        let mut message = Message {
            fade_start: 2.0,
            end: 3.0,
            ..Message::default()
        };
        assert_eq!(message.alpha(1.0), 1.0);
        message.age = 2.5;
        assert_eq!(message.alpha(1.0), 0.5);
        message.age = f32::NAN;
        assert_eq!(message.alpha(1.0), 1.0);
    }
}
