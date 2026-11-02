//! Typed 0xC8-byte event definition (spec section 3). Every byte is held by a named field, so
//! `to_bytes(from_bytes(x)) == x` for any input. Floats are kept as raw bits (NaN safe).

pub const RECORD_SIZE: usize = 0xC8;
const WORDS: usize = RECORD_SIZE / 4;

/// Trigger link (record +0x40 for A, +0x64 for B): mode, id or key, four arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Link {
    pub mode: u32,
    pub id: u32,
    pub args: [u32; 4],
}

/// Condition (record +0x7C for A, +0x90 for B): kind and four arguments (default -1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Condition {
    pub kind: u32,
    pub args: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRecord {
    /// +0x00, +0x04.
    pub head: [u32; 2],
    /// +0x08, always below 93 on the disc.
    pub verb: u32,
    /// +0x0C..=+0x34, eleven verb argument words (see `arg`).
    pub verb_args: [u32; 11],
    /// +0x38 callback id.
    pub callback: u32,
    /// +0x3C per-tick callback id.
    pub tick_callback: u32,
    pub link_a: Link,
    /// +0x58 event id of the gate definition, +0x5C gate mode.
    pub gate_id: u32,
    pub gate_mode: u32,
    /// +0x60: 0 none, 1 and, 2 or, 3 a-and-not-b.
    pub combine: u32,
    pub link_b: Link,
    pub condition_a: Condition,
    pub condition_b: Condition,
    /// +0xA4 delay seconds (f32 bits) and +0xA8 weight (f32 bits).
    pub delay_bits: u32,
    pub weight_bits: u32,
    /// +0xAC event id.
    pub event_id: u32,
    /// +0xB0 flags: 1/2/4 difficulty, 0x20 repeat, 0x40 voice priority, 0x10 keep spawned object.
    pub flags: u32,
    /// +0xB4 mode/difficulty mask, +0xB8 checkpoint mask, +0xBC restore id.
    pub mode_mask: u32,
    pub checkpoint_mask: u32,
    pub restore_id: u32,
    /// +0xC0, +0xC4: roles UNRESOLVED, preserved verbatim.
    pub unknown_c0: u32,
    pub unknown_c4: u32,
}

impl EventRecord {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != RECORD_SIZE {
            return None;
        }
        let mut words = [0u32; WORDS];
        for (word, chunk) in words.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        let mut verb_args = [0u32; 11];
        verb_args.copy_from_slice(&words[3..14]);
        let link = |at: usize| Link {
            mode: words[at],
            id: words[at + 1],
            args: [words[at + 2], words[at + 3], words[at + 4], words[at + 5]],
        };
        let condition = |at: usize| Condition {
            kind: words[at],
            args: [words[at + 1], words[at + 2], words[at + 3], words[at + 4]],
        };
        Some(Self {
            head: [words[0], words[1]],
            verb: words[2],
            verb_args,
            callback: words[14],
            tick_callback: words[15],
            link_a: link(0x40 / 4),
            gate_id: words[0x58 / 4],
            gate_mode: words[0x5C / 4],
            combine: words[0x60 / 4],
            link_b: link(0x64 / 4),
            condition_a: condition(0x7C / 4),
            condition_b: condition(0x90 / 4),
            delay_bits: words[0xA4 / 4],
            weight_bits: words[0xA8 / 4],
            event_id: words[0xAC / 4],
            flags: words[0xB0 / 4],
            mode_mask: words[0xB4 / 4],
            checkpoint_mask: words[0xB8 / 4],
            restore_id: words[0xBC / 4],
            unknown_c0: words[0xC0 / 4],
            unknown_c4: words[0xC4 / 4],
        })
    }

    pub fn to_bytes(&self) -> [u8; RECORD_SIZE] {
        let mut words = [0u32; WORDS];
        words[0] = self.head[0];
        words[1] = self.head[1];
        words[2] = self.verb;
        words[3..14].copy_from_slice(&self.verb_args);
        words[14] = self.callback;
        words[15] = self.tick_callback;
        let mut put_link = |at: usize, link: &Link| {
            words[at] = link.mode;
            words[at + 1] = link.id;
            words[at + 2..at + 6].copy_from_slice(&link.args);
        };
        put_link(0x40 / 4, &self.link_a);
        put_link(0x64 / 4, &self.link_b);
        words[0x58 / 4] = self.gate_id;
        words[0x5C / 4] = self.gate_mode;
        words[0x60 / 4] = self.combine;
        for (at, condition) in [(0x7C / 4, &self.condition_a), (0x90 / 4, &self.condition_b)] {
            words[at] = condition.kind;
            words[at + 1..at + 5].copy_from_slice(&condition.args);
        }
        words[0xA4 / 4] = self.delay_bits;
        words[0xA8 / 4] = self.weight_bits;
        words[0xAC / 4] = self.event_id;
        words[0xB0 / 4] = self.flags;
        words[0xB4 / 4] = self.mode_mask;
        words[0xB8 / 4] = self.checkpoint_mask;
        words[0xBC / 4] = self.restore_id;
        words[0xC0 / 4] = self.unknown_c0;
        words[0xC4 / 4] = self.unknown_c4;
        let mut bytes = [0u8; RECORD_SIZE];
        for (chunk, word) in bytes.chunks_exact_mut(4).zip(words) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    /// Verb argument by record offset (0x0C..=0x34, multiple of 4).
    pub fn arg(&self, offset: usize) -> u32 {
        debug_assert!((0x0C..=0x34).contains(&offset) && offset % 4 == 0);
        self.verb_args[(offset - 0x0C) / 4]
    }

    pub fn arg_f32(&self, offset: usize) -> f32 {
        f32::from_bits(self.arg(offset))
    }

    pub fn delay(&self) -> f32 {
        f32::from_bits(self.delay_bits)
    }

    pub fn weight(&self) -> f32 {
        f32::from_bits(self.weight_bits)
    }

    pub fn is_repeatable(&self) -> bool {
        self.flags & 0x20 != 0
    }

    pub fn has_voice_priority(&self) -> bool {
        self.flags & 0x40 != 0
    }

    /// Flag bits 1, 2, 4 enable difficulty 0, 1, 2; none set means all (spec section 3, +0xB0).
    pub fn enabled_on_difficulty(&self, difficulty: u8) -> bool {
        let bits = self.flags & 7;
        bits == 0 || (difficulty < 3 && bits & (1 << difficulty) != 0)
    }
}
