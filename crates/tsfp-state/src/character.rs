// SPDX-License-Identifier: GPL-3.0-or-later
//! T2077 (S3-9 of T2051): Rust-owned character records and spawn definitions. Design and honest
//! limits: docs/t-s3-character.md. Stage is Own with write-through, as in player.rs: the owned words
//! are authoritative for the declared extents, every setter stores the changed bytes into the guest
//! image in the same call, so lifted readers still see the original layout.
//!
//! The counted NPC pointer list at `0x7A3580` has its length at `0x74C37C` and holds up to `SLOTS`
//! pointers (INFERRED capacity, the region the T1996 capture used is 0x80 bytes) to `TsfpCharacter`
//! bodies of `BODY_SIZE` (0xEA0) bytes on the guest heap. `CharacterSet` owns the active prefix
//! and its bodies. Two active cells with the same pointer share one body (a write through one is
//! mirrored to the other so the owned copies cannot disagree). Inactive tail cells are not owned;
//! removal leaves the guest's old last cell untouched after decrementing the count. Field names are
//! INFERRED (TsfpCharacter in tools/data/struct_layouts.json).
use super::GuestMemory;
use super::generated_views::{TsfpCharacterOwned, TsfpCharacterView};
use super::object_pool::{ObjectPool, PoolRecordExtensionAddress, PoolRecordObjectFlags};
use super::values::GuestValue;

pub const TABLE: u32 = 0x007a_3580;
/// Count of active extension pointers in [`TABLE`] (REA T1660, 0x15BCA0/0x15BCC0).
pub const MEMBER_COUNT: u32 = 0x0074_c37c;
/// Object field holding the character extension pointer (REA T1660, 0x15BCA0/0x15BCC0).
pub const OBJECT_EXTENSION_OFFSET: u32 = 0x7c;
pub const SLOTS: usize = 32;
pub const BODY_SIZE: u32 = TsfpCharacterView::SIZE;
pub const BODY_WORDS: usize = (BODY_SIZE / 4) as usize;
/// T1743-measured zeroed extent from actor extension allocation.
pub const INITIALIZER_ZEROED_BYTES: u32 = 0x118;
/// T1743's masked branch test for applying the stored Character flag pair.
pub const PENDING_FLAGS_USE_PENDING_MASK: u32 = 0x0116_0000;

/// The two stored words of a character flag field. This preserves the source
/// layout without assigning meaning to individual bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterFlagWords {
    pub low: u32,
    pub high: u32,
}

impl CharacterFlagWords {
    /// T1743 observes this fallback pair at the pending-flag application site.
    /// Its game-level meaning remains unknown.
    pub const DEFAULT_APPLICATION: Self = Self {
        low: 0x0800_0000,
        high: 0,
    };
}

/// The character-local state fields consumed at controller entry. Their
/// labels follow the bounded static control-flow contract in T2435.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterTickState {
    pub current: u32,
    pub pending: i32,
    pub previous: u32,
    pub transition_time: f32,
}

/// Controller-entry state with two address-labelled values seen in the T1661
/// behavior tick. The byte at +0x2c and +0xa90 word remain raw here so this
/// wider snapshot does not assume their field semantics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterBehaviorTickState {
    pub controller: CharacterTickState,
    pub byte_2c: u8,
    pub word_a90: u32,
}

/// Measured control bits in C+0x2c. Other bits stay opaque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterBehaviorFlags {
    pub raw: u8,
}

impl CharacterBehaviorFlags {
    /// T1743 observes mask 0x04 skip pending-state application.
    pub const SKIP_PENDING_APPLICATION_MASK: u8 = 1 << 2;
    /// T1743 observes bit 7 controlling the accumulator branch; polarity is
    /// deliberately represented only as whether the bit is set.
    pub const ACCUMULATOR_CONTROL_MASK: u8 = 1 << 7;

    pub fn skip_pending_application(self) -> bool {
        self.raw & Self::SKIP_PENDING_APPLICATION_MASK != 0
    }

    pub fn with_skip_pending_application(mut self, enabled: bool) -> Self {
        if enabled {
            self.raw |= Self::SKIP_PENDING_APPLICATION_MASK;
        } else {
            self.raw &= !Self::SKIP_PENDING_APPLICATION_MASK;
        }
        self
    }

    pub fn accumulator_control_bit(self) -> bool {
        self.raw & Self::ACCUMULATOR_CONTROL_MASK != 0
    }

    pub fn with_accumulator_control_bit(mut self, enabled: bool) -> Self {
        if enabled {
            self.raw |= Self::ACCUMULATOR_CONTROL_MASK;
        } else {
            self.raw &= !Self::ACCUMULATOR_CONTROL_MASK;
        }
        self
    }
}

/// Float stored at Character+0xa90. T1743 observes a flag-controlled
/// accumulator there; this typed view does not assign units or update cadence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterFlagControlledAccumulator {
    pub value: f32,
}

/// Raw guest words stored in four character target/reference fields. These
/// remain ABI values; the snapshot does not imply object identity or lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterTargetWords {
    pub melee_target: u32,
    pub alert_target: u32,
    pub threatening_object: u32,
    pub pickup_target: u32,
}

/// Raw Object pointer word stored at Character+0. The field is measured as a
/// link to TsfpObject, but this value does not establish object identity,
/// generation, or lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterObjectReference {
    pub raw: u32,
}

/// Signed word at Character+8. Its use as an index is measured, but its
/// domain and relationship to CharacterSet slots are unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterIndexWord {
    pub raw: i32,
}

/// Four inferred class-mask words in the character extension. Their bit
/// assignments remain opaque; the names identify only the declared ABI words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterClassMaskWords {
    pub class_low: u32,
    pub class_high: u32,
    pub alternate_class_low: u32,
    pub alternate_class_high: u32,
}

/// Raw word in the inferred Character+0xc6c readiness-flags field. T1660
/// observes bit 1 gate a call with selector 0x112; other bits remain opaque.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterReadinessFlags {
    pub raw: u32,
}

impl CharacterReadinessFlags {
    /// T1660 observes this bit tested before the 0x48034(actor, 0x112) call.
    pub const SELECTOR_0X112_GATE_MASK: u32 = 1 << 1;

    pub fn selector_0x112_gate(self) -> bool {
        self.raw & Self::SELECTOR_0X112_GATE_MASK != 0
    }

    pub fn with_selector_0x112_gate(mut self, enabled: bool) -> Self {
        if enabled {
            self.raw |= Self::SELECTOR_0X112_GATE_MASK;
        } else {
            self.raw &= !Self::SELECTOR_0X112_GATE_MASK;
        }
        self
    }
}

/// Opaque ABI word at Character+0x94. T1743 infers a selected loadout weapon
/// role; this is distinct from Object+0x94 and establishes no weapon identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterLoadoutWeaponWord {
    pub raw: u32,
}

/// Raw bytes in the inferred Character+0x50 class-label buffer. T1743 observes
/// a 0x1e-byte copy but does not establish text encoding or termination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterClassLabelBytes {
    pub bytes: [u8; 0x1e],
}

/// Character damage and combat-history fields copied with their generated-view
/// widths. The names remain INFERRED ABI labels; raw ID and stamp words do not
/// become identity, timing, or lifetime contracts here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterDamageHistoryState {
    pub seconds_since_damage: f32,
    pub last_health_damage: f32,
    pub threat_stamp: u32,
    pub threat_id: u32,
    pub melee_stamp: u32,
    pub melee_hit_count: u16,
}

/// Complete character extension body as raw dwords. Unknown fields and
/// pointer-like words retain their ABI values without gaining identity or
/// lifetime semantics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterBodyState {
    pub words: [u32; BODY_WORDS],
}

/// Raw word at Character extension +0x2C. T1660 measures bit 2 being tested
/// and cleared by the reactivation path; no other bit meanings are assigned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterExtensionWord2C {
    pub word: u32,
}

/// Opaque word at actor extension +0x34, populated by measured initializer
/// branches. The stored values are not assigned gameplay meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerField34(pub u32);

/// Opaque model-dependent state word at actor extension +0x00. T1743 measures
/// three model-key outputs without establishing names for the states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerModelState(pub u32);

/// Position copied to actor extension +0x04/+0x08/+0x0C by the measured
/// initializer. The 0x3B3 Y adjustment is represented as a value, not a
/// semantic actor transform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Source values copied from Object+0xC0/+0x70/+0xC8 and scaled by the
/// measured actor initializer before writing extension +0x28/+0x2C/+0x30.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerRotationInputs {
    pub object_c0: f32,
    pub object_70: f32,
    pub object_c8: f32,
}

/// Opaque scaled rotation outputs at actor extension +0x28/+0x2C/+0x30.
/// T1743 records the copies and common multiplier without proving their units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerRotations {
    pub field_28: f32,
    pub field_2c: f32,
    pub field_30: f32,
}

/// Opaque float outputs at actor extension +0x4C/+0x5C/+0x60. T1743 records
/// their source constants and model-key branches without assigning meanings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerParameters {
    pub field_4c: f32,
    pub field_5c: f32,
    pub field_60: f32,
}

/// Opaque RNG-derived actor initializer outputs at extension +0x74/+0x90/+0x94.
/// T1743 observes +0x90 and +0x94 receiving the same sample; it does not
/// establish the random generator's state or sampling algorithm.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerRandomOutputs {
    pub field_74: f32,
    pub field_90: f32,
    pub field_94: f32,
}

/// Caller-supplied values from the measured initializer's random source. The
/// +0x90/+0x94 sample is shared by both extension fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerRandomSamples {
    pub field_74_sample: f32,
    pub field_90_94_sample: f32,
}

/// Caller-owned values loaded from the initializer's measured constant
/// addresses; kept explicit so this writer does not invent guest constants.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerParameterSources {
    pub field_4c: f32,
    pub field_5c: f32,
    pub field_60: f32,
    pub field_5c_increment_for_553: f32,
    pub field_60_replacement_for_553: f32,
}

/// Opaque outputs at actor extension +0x64/+0xA8 for the measured model-0x482
/// default initializer branch. The values have no assigned gameplay meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerModel482Outputs {
    pub field_64: f32,
    pub field_a8: f32,
}

/// Caller-loaded constants used by the measured model-0x482 default branch.
/// This keeps the source addresses and any constant interpretation at the
/// caller boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerModel482Sources {
    pub field_64_source: f32,
    pub field_a8_adjustment: f32,
}

/// The opaque initializer dword at actor extension +0x38. T1743 measures
/// conditional OR operations here without establishing the field's meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerField38(pub u32);

/// The opaque descriptor copy at keyed actor buffer +0x3C. T1743 observes it
/// only when descriptor K is non-null; its meaning remains unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerField3C(pub u32);

/// Inputs selected by the measured actor initializer for its +0x34/+0x38 and
/// optional +0x3C writes. `model_key` is the measured 16-bit Object+0x94 value;
/// K+2/K+3 and optional K+0x28 feed distinct branches/copies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerBranchInput {
    pub model_key: u16,
    pub descriptor_k2: u8,
    pub descriptor_k3: u8,
    pub descriptor_k28: Option<u32>,
}

/// The two opaque actor-extension words covered by the measured initializer
/// slice. This does not represent a complete actor initialization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterInitializerMeasuredWords {
    pub field_34: CharacterInitializerField34,
    pub field_38: CharacterInitializerField38,
    pub field_3c: Option<CharacterInitializerField3C>,
}

/// Explicit source values for the currently measured actor-initializer slice.
/// These inputs deliberately omit allocation, Object+0x68 adjustment, and
/// other writes outside the T1743-backed CharacterSet contract.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerMeasuredInputs {
    pub branch: CharacterInitializerBranchInput,
    pub object_position: CharacterInitializerPosition,
    pub position_y_adjustment: f32,
    pub rotation_inputs: CharacterInitializerRotationInputs,
    pub rotation_scale: f32,
    pub parameter_sources: CharacterInitializerParameterSources,
}

/// Typed results of the measured CharacterSet initializer writers. This is a
/// bounded field subset, not a complete actor-initialization state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterInitializerMeasuredFields {
    pub model_state: CharacterInitializerModelState,
    pub position: CharacterInitializerPosition,
    pub rotations: CharacterInitializerRotations,
    pub parameters: CharacterInitializerParameters,
    pub words: CharacterInitializerMeasuredWords,
}

/// The measured three-float `aim_point` field at Character+0xBB4. This
/// snapshot records its ABI shape only; actor ownership and update timing are
/// still unresolved by the REA checkpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterAimPoint {
    pub coordinates: [f32; 3],
}

/// The inferred aim-height scalar at Character+0xBC0. T1743 records its
/// mode-A use, but this value does not establish actor ownership or timing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterAimHeight {
    pub height: f32,
}

/// Inferred f32 field at Character+0x9FC. T1743 records aim/random-scale
/// accesses, but this snapshot does not resolve its meaning or update cadence.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterAimScale {
    pub scale: f32,
}

/// Separate signed selectors at Character+0x48 and +0x4A. T1743 labels +0x48
/// an INFERRED type/model selector and identifies +0x4A as a second selector;
/// neither field is joined here to the object model ID at O+0x94.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterSelectorWords {
    pub type_model_selector: i16,
    pub secondary_selector: i16,
}

/// The measured cached-target XYZ triple at Character+0x30. The field label
/// follows the generated ABI view; the value alone does not identify a target
/// object or establish awareness ownership or update timing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterCachedTargetPosition {
    pub coordinates: [f32; 3],
}

/// The measured route and goal ID words at Character+0x678 and +0x684.
/// The IDs remain raw values; this pair does not establish a route owner,
/// object identity, or update cadence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterRouteGoalState {
    pub route_id: u32,
    pub goal_id: u32,
}

/// Raw dwords at Character+0x9A8 and +0x9AC. T1743 records both being cleared
/// before a route query, but does not establish their further meanings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterRouteQueryPreclearWords {
    pub at_9a8: u32,
    pub at_9ac: u32,
}

/// Raw pointer word at Character+0xBF0. T1743 observes a callback/parameter
/// record there, but this value does not own or extend the pointee's lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterCallbackRecordReference {
    pub raw: u32,
}

/// The signed word at Character+0x8. T1743 observes signed comparisons against
/// the player count and a value from 0x74C388; the exact domain remains unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterSignedIndexWord {
    pub raw: i32,
}

/// Raw dwords at Character+0xB30 and +0xB34. T1743 names timer accesses but
/// does not establish units, expiry rules, or the owning tick domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterTimerWords {
    pub at_b30: u32,
    pub at_b34: u32,
}

/// Raw dwords at Character+0xB68 and +0xB6C. T1743 infers contact
/// interval/counter labels from the observed access, but units, cadence, and
/// ownership remain unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterContactWords {
    pub at_b68: u32,
    pub at_b6c: u32,
}

/// Caller-supplied tick value observed at Character+0xB68 in the measured
/// melee contact prologue. Its clock domain and wrap policy are unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterContactTick(pub u32);

/// Caller-supplied result of the measured contact-window branch and the tick
/// value written after that branch. Tick comparison and clock semantics remain
/// outside this state transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterContactWindowUpdate {
    pub tick: CharacterContactTick,
    pub clear_counter: bool,
}

/// Low 16-bit value at Character+0xB6C. The measured melee contact path
/// conditionally clears this word; its count meaning and reset/re-arm rules
/// remain unresolved. The adjacent +0xB6E word is independent state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterContactCounter(pub u16);

/// Accepted attacker counter stored at target Character+0xB6E. The measured
/// comparison is unsigned; reset and re-arm writers remain unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterAcceptedContactCounter(pub u16);

/// Raw dwords in the inferred three-word region at Character+0xAF8..+0xB00.
/// The navigation-point label remains INFERRED; these words do not identify
/// an object or establish navigation ownership or update timing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterNavPointWords {
    pub at_af8: u32,
    pub at_afc: u32,
    pub at_b00: u32,
}

/// Inferred turn-threshold floats at Character+0xB18 and +0xB28. The names
/// follow the sparse T1743 field map; this snapshot does not establish units,
/// update cadence, or actor lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharacterTurnThresholds {
    pub threshold: f32,
    pub delta: f32,
}

/// Outcome of consuming one character-local pending state request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterTickEffect {
    /// A differing nonnegative request became the current state.
    StateChanged { from: u32, to: u32 },
    /// A negative sentinel or a request for the current state was cleared.
    RequestCleared { current: u32 },
    /// The C+0x2c behavior bit bypassed pending-state application.
    PendingApplicationSkipped,
}

impl CharacterTickState {
    /// Resolve the controller-entry request in Rust-owned state. The caller
    /// writes the returned state through using CharacterSet's field setters.
    pub fn consume_pending_request(mut self) -> (Self, CharacterTickEffect) {
        let effect = if self.pending >= 0 && self.pending as u32 != self.current {
            let from = self.current;
            let to = self.pending as u32;
            self.current = to;
            self.previous = from;
            self.transition_time = 0.0;
            CharacterTickEffect::StateChanged { from, to }
        } else {
            CharacterTickEffect::RequestCleared {
                current: self.current,
            }
        };
        self.pending = -1;
        (self, effect)
    }
}

/// The 42 declared fields of `TsfpCharacter`: (offset, byte size, name). Drift-tested against
/// tools/data/struct_layouts.json.
pub const FIELDS: [(u32, u32, &str); 42] = [
    (0x0, 4, "object"),
    (0x4, 4, "melee_target"),
    (0x8, 4, "index_word"),
    (0x10, 4, "class_mask_lo"),
    (0x14, 4, "class_mask_hi"),
    (0x18, 4, "alt_class_lo"),
    (0x1c, 4, "alt_class_hi"),
    (0x20, 4, "state"),
    (0x28, 1, "flags_28"),
    (0x2c, 1, "flags_2c"),
    (0x30, 12, "cached_target_position"),
    (0x48, 2, "model_selector"),
    (0x4a, 2, "model_selector_b"),
    (0x50, 0x1e, "class_label"),
    (0x78, 4, "armor"),
    (0x94, 4, "loadout_weapon"),
    (0x9d0, 4, "pending_state"),
    (0x9d4, 4, "previous_state"),
    (0x9d8, 4, "transition_time"),
    (0x9fc, 4, "aim_scale"),
    (0xa90, 4, "flag_accumulator"),
    (0xaa8, 4, "goal_flags_lo"),
    (0xaac, 4, "goal_flags_hi"),
    (0xab8, 4, "pending_flags_lo"),
    (0xabc, 4, "pending_flags_hi"),
    (0xac8, 4, "alert_target"),
    (0xaec, 4, "threatening_object"),
    (0xaf0, 4, "pickup_target"),
    (0xaf4, 4, "pickup_distance"),
    (0xaf8, 12, "nav_point"),
    (0xb18, 4, "turn_threshold"),
    (0xb28, 4, "turn_delta"),
    (0xb44, 4, "seconds_since_damage"),
    (0xb48, 4, "last_health_damage"),
    (0xb60, 4, "threat_stamp"),
    (0xb64, 4, "threat_id"),
    (0xb68, 4, "melee_stamp"),
    (0xb6c, 2, "melee_hit_count"),
    (0xbb4, 12, "aim_point"),
    (0xbc0, 4, "aim_height"),
    (0xbf0, 4, "callback_record"),
    (0xc6c, 4, "readiness_flags"),
];

/// The first owned location that differs between the owned value and the guest image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharDiff {
    /// Count of active pointers in the NPC table.
    Count,
    /// NPC table cell of this slot.
    Cell(usize),
    /// Body of this slot, byte offset of the differing dword.
    Body(usize, u32),
}

/// Byte window over one owned body, so the generated view code (`TsfpCharacterView::load`, field
/// setters) runs on the owned words. Reads outside the body give 0, writes outside are dropped.
pub struct BodyMem<'a> {
    pub base: u32,
    pub words: &'a mut [u32; BODY_WORDS],
}
impl GuestMemory for BodyMem<'_> {
    fn read8(&self, a: u32) -> u8 {
        let o = a.wrapping_sub(self.base);
        if o < BODY_SIZE {
            self.words[(o / 4) as usize].to_le_bytes()[(o % 4) as usize]
        } else {
            0
        }
    }
    fn write8(&mut self, a: u32, v: u8) {
        let o = a.wrapping_sub(self.base);
        if o < BODY_SIZE {
            let w = &mut self.words[(o / 4) as usize];
            let mut b = w.to_le_bytes();
            b[(o % 4) as usize] = v;
            *w = u32::from_le_bytes(b);
        }
    }
}

/// Owner of the NPC table and the character bodies behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterSet {
    /// Raw guest count. Values above `SLOTS` are preserved but cannot be mutated by this bounded owner.
    pub member_count: u32,
    pub ptrs: [u32; SLOTS],
    pub bodies: [[u32; BODY_WORDS]; SLOTS],
}

/// Result of the measured reactivation flag update and optional CharacterSet
/// membership append. This reports only whether the bounded append succeeded;
/// it does not establish actor identity, generation, or lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterReactivationResult {
    pub object_flags: PoolRecordObjectFlags,
    pub membership: CharacterReactivationMembership,
}

/// Outcome of the caller-selected membership branch in the measured
/// reactivation sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterReactivationMembership {
    BranchNotTaken,
    Appended { slot: usize },
    AppendRejected,
}

/// Result of the bounded `0x15DDA0` retirement seam. `NotMember` leaves the
/// Object+0x7C pointer intact; it does not authorize native lifetime changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterRetirementMembership {
    Removed { slot: usize },
    NotMember,
}

impl CharacterSet {
    pub const EMPTY: Self = Self {
        member_count: 0,
        ptrs: [0; SLOTS],
        bodies: [[0; BODY_WORDS]; SLOTS],
    };

    /// Mirror: copies the count, active pointer prefix, and active bodies out of the guest image.
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let mut s = Self::EMPTY;
        s.member_count = m.read32(MEMBER_COUNT);
        for i in 0..s.active_len() {
            s.ptrs[i] = m.read32(TABLE + 4 * i as u32);
        }
        s.load_bodies(m);
        s
    }

    /// Re-reads the bodies at the current owned pointers (a hybrid tick after a lifted writer).
    pub fn load_bodies<M: GuestMemory>(&mut self, m: &M) {
        for i in 0..self.active_len() {
            if self.ptrs[i] != 0 {
                for (j, w) in self.bodies[i].iter_mut().enumerate() {
                    *w = m.read32(self.ptrs[i].wrapping_add(j as u32 * 4));
                }
            }
        }
    }

    /// Writes the count, active pointer prefix, and its bodies. Inactive tail cells are not owned
    /// and remain as found in guest memory.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        m.write32(MEMBER_COUNT, self.member_count);
        for i in 0..self.active_len() {
            m.write32(TABLE + 4 * i as u32, self.ptrs[i]);
            if self.ptrs[i] != 0 {
                for (j, w) in self.bodies[i].iter().enumerate() {
                    m.write32(self.ptrs[i].wrapping_add(j as u32 * 4), *w);
                }
            }
        }
    }

    /// First owned location whose guest bytes differ.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<CharDiff> {
        if m.read32(MEMBER_COUNT) != self.member_count {
            return Some(CharDiff::Count);
        }
        for i in 0..self.active_len() {
            if m.read32(TABLE + 4 * i as u32) != self.ptrs[i] {
                return Some(CharDiff::Cell(i));
            }
        }
        for i in 0..self.active_len() {
            if self.ptrs[i] == 0 {
                continue;
            }
            for (j, w) in self.bodies[i].iter().enumerate() {
                if m.read32(self.ptrs[i].wrapping_add(j as u32 * 4)) != *w {
                    return Some(CharDiff::Body(i, j as u32 * 4));
                }
            }
        }
        None
    }

    fn active_len(&self) -> usize {
        (self.member_count as usize).min(SLOTS)
    }

    /// Number of active pointers in the bounded owned prefix.
    pub fn live(&self) -> usize {
        self.active_len()
    }

    /// Slot whose body starts at `ptr` (the character pointer `Object+0x7C` points to), if owned.
    pub fn slot_of(&self, ptr: u32) -> Option<usize> {
        (ptr != 0)
            .then(|| {
                self.ptrs[..self.active_len()]
                    .iter()
                    .position(|p| *p == ptr)
            })
            .flatten()
    }

    pub fn body_word(&self, slot: usize, offset: u32) -> u32 {
        self.bodies[slot][(offset / 4) as usize]
    }

    /// Copies one live extension body into a typed, Rust-owned snapshot.
    pub fn body_state(&self, slot: usize) -> Option<CharacterBodyState> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterBodyState {
            words: self.bodies[slot],
        })
    }

    /// Replaces a complete live body and writes it through to the guest image.
    /// Existing slots that share this raw body pointer receive the same mirror
    /// so the owned aliases continue to agree. This does not retain a pointer
    /// across ticks or define actor identity, generation, or retirement.
    pub fn set_body_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        state: CharacterBodyState,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        for alias in 0..SLOTS {
            if self.ptrs[alias] == ptr {
                self.bodies[alias] = state.words.clone();
            }
        }
        for (word, value) in state.words.iter().copied().enumerate() {
            m.write32(ptr.wrapping_add(word as u32 * 4), value);
        }
        true
    }

    /// Reads the raw dword at extension +0x2C from Rust-owned character state.
    pub fn extension_word_2c(&self, slot: usize) -> Option<CharacterExtensionWord2C> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterExtensionWord2C {
            word: self.body_word(slot, 0x2c),
        })
    }

    /// Clears bit 2 at extension +0x2C and mirrors the write through all slots
    /// aliasing this body. This implements only the measured word update; it
    /// does not publish membership or establish actor lifetime semantics.
    pub fn clear_extension_word_2c_bit2<M: GuestMemory>(&mut self, m: &mut M, slot: usize) -> bool {
        let Some(word) = self.extension_word_2c(slot) else {
            return false;
        };
        self.set_word(m, slot, 0x2c, word.word & !4)
    }

    /// Reads and clears bit 2 at extension +0x2C from Rust-owned character
    /// state, returning its prior value. `None` means the slot has no owned
    /// extension. This preserves the observed test/clear operation without
    /// assigning a meaning to the bit or deciding the caller's branch.
    pub fn take_extension_word_2c_bit2<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
    ) -> Option<bool> {
        let word = self.extension_word_2c(slot)?;
        let was_set = word.word & 4 != 0;
        self.set_word(m, slot, 0x2c, word.word & !4)
            .then_some(was_set)
    }

    /// Reads the opaque initializer field at actor extension +0x34 from owned
    /// state. T1743 measures model-key-dependent writes here but does not name
    /// the field's meaning.
    pub fn initializer_field_34(&self, slot: usize) -> Option<CharacterInitializerField34> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerField34(self.body_word(slot, 0x34)))
    }

    /// Reads the measured model-dependent initializer word at extension +0x00.
    pub fn initializer_model_state(&self, slot: usize) -> Option<CharacterInitializerModelState> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerModelState(self.body_word(slot, 0)))
    }

    /// Reads the measured initializer position triple from Rust-owned state.
    pub fn initializer_position(&self, slot: usize) -> Option<CharacterInitializerPosition> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerPosition {
            x: f32::from_bits(self.body_word(slot, 0x04)),
            y: f32::from_bits(self.body_word(slot, 0x08)),
            z: f32::from_bits(self.body_word(slot, 0x0c)),
        })
    }

    /// Reads the opaque scaled rotation triple written by the measured actor
    /// initializer at extension +0x28/+0x2C/+0x30.
    pub fn initializer_rotations(&self, slot: usize) -> Option<CharacterInitializerRotations> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerRotations {
            field_28: f32::from_bits(self.body_word(slot, 0x28)),
            field_2c: f32::from_bits(self.body_word(slot, 0x2c)),
            field_30: f32::from_bits(self.body_word(slot, 0x30)),
        })
    }

    /// Reads the pre-adjustment position snapshot at actor extension
    /// +0x98/+0x9C/+0xA0.
    pub fn initializer_source_position(&self, slot: usize) -> Option<CharacterInitializerPosition> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerPosition {
            x: f32::from_bits(self.body_word(slot, 0x98)),
            y: f32::from_bits(self.body_word(slot, 0x9c)),
            z: f32::from_bits(self.body_word(slot, 0xa0)),
        })
    }

    /// Reads the opaque measured initializer float outputs at +0x4C/+0x5C/
    /// +0x60 from Rust-owned state.
    pub fn initializer_parameters(&self, slot: usize) -> Option<CharacterInitializerParameters> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerParameters {
            field_4c: f32::from_bits(self.body_word(slot, 0x4c)),
            field_5c: f32::from_bits(self.body_word(slot, 0x5c)),
            field_60: f32::from_bits(self.body_word(slot, 0x60)),
        })
    }

    /// Reads the opaque RNG-derived initializer outputs at +0x74/+0x90/+0x94
    /// from Rust-owned character state.
    pub fn initializer_random_outputs(
        &self,
        slot: usize,
    ) -> Option<CharacterInitializerRandomOutputs> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerRandomOutputs {
            field_74: f32::from_bits(self.body_word(slot, 0x74)),
            field_90: f32::from_bits(self.body_word(slot, 0x90)),
            field_94: f32::from_bits(self.body_word(slot, 0x94)),
        })
    }

    /// Reads the opaque +0x64/+0xA8 outputs from Rust-owned character state.
    pub fn initializer_model_482_outputs(
        &self,
        slot: usize,
    ) -> Option<CharacterInitializerModel482Outputs> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerModel482Outputs {
            field_64: f32::from_bits(self.body_word(slot, 0x64)),
            field_a8: f32::from_bits(self.body_word(slot, 0xa8)),
        })
    }

    /// Writes only T1743's measured extension +0x00 outputs: model 0x553 maps
    /// to 2, 0x482 to 1, and 0x3B3 to 0. Unsupported keys leave state and guest
    /// memory untouched; the numeric outputs are not assigned semantic names.
    pub fn apply_measured_initializer_model_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u16,
    ) -> Option<CharacterInitializerModelState> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let value = match model_key {
            0x0553 => 2,
            0x0482 => 1,
            0x03b3 => 0,
            _ => return None,
        };
        self.set_word(m, slot, 0, value)
            .then_some(CharacterInitializerModelState(value))
    }

    /// Writes T1743's measured initializer position outputs at extension
    /// +0x04/+0x08/+0x0C. All three known model keys copy the supplied Object
    /// position; model 0x3B3 additionally subtracts `y_adjustment` from Y.
    /// The separate Object+0x68 mutation observed on that branch remains the
    /// ObjectPool caller's responsibility. Unknown keys and absent slots do
    /// not mutate owned state or guest memory.
    pub fn apply_measured_initializer_position<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u16,
        object_position: CharacterInitializerPosition,
        y_adjustment: f32,
    ) -> Option<CharacterInitializerPosition> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let mut value = match model_key {
            0x0553 | 0x0482 | 0x03b3 => object_position,
            _ => return None,
        };
        if model_key == 0x03b3 {
            value.y -= y_adjustment;
        }
        let wrote = self.set_word(m, slot, 0x04, value.x.to_bits())
            & self.set_word(m, slot, 0x08, value.y.to_bits())
            & self.set_word(m, slot, 0x0c, value.z.to_bits())
            & self.set_word(m, slot, 0x98, object_position.x.to_bits())
            & self.set_word(m, slot, 0x9c, object_position.y.to_bits())
            & self.set_word(m, slot, 0xa0, object_position.z.to_bits());
        wrote.then_some(value)
    }

    /// Applies T1743's measured common scaling from Object+0xC0/+0x70/+0xC8
    /// to actor extension +0x28/+0x2C/+0x30. `scale` is supplied by the caller
    /// from the measured constant; units and rotation meaning remain unproven.
    /// Missing slots do not mutate owned or guest state.
    pub fn apply_measured_initializer_rotations<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        inputs: CharacterInitializerRotationInputs,
        scale: f32,
    ) -> Option<CharacterInitializerRotations> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let value = CharacterInitializerRotations {
            field_28: inputs.object_c0 * scale,
            field_2c: inputs.object_70 * scale,
            field_30: inputs.object_c8 * scale,
        };
        let wrote = self.set_word(m, slot, 0x28, value.field_28.to_bits())
            & self.set_word(m, slot, 0x2c, value.field_2c.to_bits())
            & self.set_word(m, slot, 0x30, value.field_30.to_bits());
        wrote.then_some(value)
    }

    /// Applies T1743's measured opaque initializer outputs at +0x4C/+0x5C/
    /// +0x60. All three known model keys use the supplied base values; model
    /// 0x553 increments +0x5C and replaces +0x60, while 0x3B3 zeros +0x60.
    /// Unknown keys and absent slots do not mutate owned state or guest memory.
    pub fn apply_measured_initializer_parameters<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u16,
        sources: CharacterInitializerParameterSources,
    ) -> Option<CharacterInitializerParameters> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let value = match model_key {
            0x0553 => CharacterInitializerParameters {
                field_4c: sources.field_4c,
                field_5c: sources.field_5c + sources.field_5c_increment_for_553,
                field_60: sources.field_60_replacement_for_553,
            },
            0x0482 => CharacterInitializerParameters {
                field_4c: sources.field_4c,
                field_5c: sources.field_5c,
                field_60: sources.field_60,
            },
            0x03b3 => CharacterInitializerParameters {
                field_4c: sources.field_4c,
                field_5c: sources.field_5c,
                field_60: 0.0,
            },
            _ => return None,
        };
        let wrote = self.set_word(m, slot, 0x4c, value.field_4c.to_bits())
            & self.set_word(m, slot, 0x5c, value.field_5c.to_bits())
            & self.set_word(m, slot, 0x60, value.field_60.to_bits());
        wrote.then_some(value)
    }

    /// Writes T1743's measured RNG-derived initializer outputs when the model
    /// branch is 0x3B3 or 0x482 with descriptor K+3 == 0x42. Samples are
    /// supplied by the caller because the RNG state/algorithm is not part of
    /// this field contract; the observed +0x90 sample is copied to +0x94.
    /// Unsupported branches and absent slots leave owned and guest state
    /// untouched.
    pub fn apply_measured_initializer_random_outputs<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u16,
        descriptor_k3: u8,
        samples: CharacterInitializerRandomSamples,
    ) -> Option<CharacterInitializerRandomOutputs> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 || !(model_key == 0x03b3 || (model_key == 0x0482 && descriptor_k3 == 0x42)) {
            return None;
        }
        let value = CharacterInitializerRandomOutputs {
            field_74: samples.field_74_sample,
            field_90: samples.field_90_94_sample,
            field_94: samples.field_90_94_sample,
        };
        let wrote = self.set_word(m, slot, 0x74, value.field_74.to_bits())
            & self.set_word(m, slot, 0x90, value.field_90.to_bits())
            & self.set_word(m, slot, 0x94, value.field_94.to_bits());
        wrote.then_some(value)
    }

    /// Applies T1743's model-0x482 default-branch writes to extension +0x64
    /// and +0xA8. The caller selects the measured default branch and supplies
    /// the two loaded constants; +0xA8 is formed by subtracting its constant
    /// from the initializer's observed zero value. This does not model branch
    /// selection, initializer order, allocation, or actor lifetime.
    pub fn apply_measured_initializer_model_482_default_outputs<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        sources: CharacterInitializerModel482Sources,
    ) -> Option<CharacterInitializerModel482Outputs> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let value = CharacterInitializerModel482Outputs {
            field_64: sources.field_64_source,
            field_a8: 0.0 - sources.field_a8_adjustment,
        };
        let wrote = self.set_word(m, slot, 0x64, value.field_64.to_bits())
            & self.set_word(m, slot, 0xa8, value.field_a8.to_bits());
        wrote.then_some(value)
    }

    /// Applies only the measured initializer branch for extension +0x34:
    /// model key 0x553 writes 0xA, 0x3B3 writes 6, and 0x482 selects 0/4/3
    /// from descriptor byte K+3. Unsupported keys leave owned state and guest
    /// memory untouched. Other initializer fields and allocation semantics
    /// remain outside this operation.
    pub fn apply_measured_initializer_field_34<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u32,
        descriptor_byte_3: u8,
    ) -> Option<CharacterInitializerField34> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let value = match model_key {
            0x553 => 0xA,
            0x3B3 => 6,
            0x482 => match descriptor_byte_3 {
                0x50 => 0,
                0x42 => 4,
                _ => 3,
            },
            _ => return None,
        };
        self.set_word(m, slot, 0x34, value)
            .then_some(CharacterInitializerField34(value))
    }

    /// Reads the opaque initializer dword at actor extension +0x38 from
    /// Rust-owned character state.
    pub fn initializer_field_38(&self, slot: usize) -> Option<CharacterInitializerField38> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerField38(self.body_word(slot, 0x38)))
    }

    /// Applies the measured initializer ORs at extension +0x38. T1743 observes
    /// bit 3 ORed when descriptor byte K+2 is 0x47, and bits 0 and 2 ORed on
    /// the model-key 0x482 default branch. Existing bits are preserved; this
    /// does not assign meaning to the word or reconstruct other initializer
    /// branches.
    pub fn apply_measured_initializer_field_38<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        model_key: u32,
        descriptor_byte_2: u8,
    ) -> Option<CharacterInitializerField38> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let or_mask = (if descriptor_byte_2 == 0x47 { 8 } else { 0 })
            | (if model_key == 0x482 { 5 } else { 0 });
        let value = self.body_word(slot, 0x38) | or_mask;
        self.set_word(m, slot, 0x38, value)
            .then_some(CharacterInitializerField38(value))
    }

    /// Reads the optional opaque descriptor copy at keyed actor buffer +0x3C.
    pub fn initializer_field_3c(&self, slot: usize) -> Option<CharacterInitializerField3C> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterInitializerField3C(self.body_word(slot, 0x3c)))
    }

    /// Copies K+0x28 into keyed actor buffer +0x3C. The caller supplies a
    /// value only when the measured initializer's K pointer is non-null.
    pub fn apply_measured_initializer_field_3c<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        descriptor_word: u32,
    ) -> Option<CharacterInitializerField3C> {
        self.set_word(m, slot, 0x3c, descriptor_word)
            .then_some(CharacterInitializerField3C(descriptor_word))
    }

    /// Applies the measured +0x34 and +0x38 initializer branches together.
    /// Only the observed model keys are accepted, and both field operations
    /// preserve their separate overwrite/OR behavior. Allocation, other
    /// initializer fields, and actor lifetime remain outside this slice.
    pub fn apply_measured_initializer_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        input: CharacterInitializerBranchInput,
    ) -> Option<CharacterInitializerMeasuredWords> {
        if !matches!(input.model_key, 0x03b3 | 0x0482 | 0x0553) {
            return None;
        }
        let model_key = u32::from(input.model_key);
        let field_34 =
            self.apply_measured_initializer_field_34(m, slot, model_key, input.descriptor_k3)?;
        let field_38 =
            self.apply_measured_initializer_field_38(m, slot, model_key, input.descriptor_k2)?;
        let field_3c = input.descriptor_k28.and_then(|descriptor_word| {
            self.apply_measured_initializer_field_3c(m, slot, descriptor_word)
        });
        Some(CharacterInitializerMeasuredWords {
            field_34,
            field_38,
            field_3c,
        })
    }

    /// Applies the currently measured initializer writers as one typed
    /// CharacterSet operation. The fields are independent writes; this method
    /// does not assert their original execution order or include ObjectPool
    /// mutations, extension allocation, or actor-lifetime behavior.
    pub fn apply_measured_initializer_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        input: CharacterInitializerMeasuredInputs,
    ) -> Option<CharacterInitializerMeasuredFields> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 || !matches!(input.branch.model_key, 0x03b3 | 0x0482 | 0x0553) {
            return None;
        }

        let model_state =
            self.apply_measured_initializer_model_state(m, slot, input.branch.model_key)?;
        let position = self.apply_measured_initializer_position(
            m,
            slot,
            input.branch.model_key,
            input.object_position,
            input.position_y_adjustment,
        )?;
        let rotations = self.apply_measured_initializer_rotations(
            m,
            slot,
            input.rotation_inputs,
            input.rotation_scale,
        )?;
        let parameters = self.apply_measured_initializer_parameters(
            m,
            slot,
            input.branch.model_key,
            input.parameter_sources,
        )?;
        let words = self.apply_measured_initializer_words(m, slot, input.branch)?;

        Some(CharacterInitializerMeasuredFields {
            model_state,
            position,
            rotations,
            parameters,
            words,
        })
    }

    /// Zeroes the measured 0x118-byte extension prefix, then applies the
    /// bounded typed initializer fields. This composes the observed
    /// allocation-initialization boundary without allocating storage,
    /// changing ObjectPool state, or asserting the order among field writes.
    /// Invalid model keys and absent slots are rejected before mutation.
    pub fn initialize_measured_extension_prefix_then_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        input: CharacterInitializerMeasuredInputs,
    ) -> Option<(u32, CharacterInitializerMeasuredFields)> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 || !matches!(input.branch.model_key, 0x03b3 | 0x0482 | 0x0553) {
            return None;
        }
        let zeroed_bytes = self.zero_measured_initializer_prefix(m, slot)?;
        let fields = self.apply_measured_initializer_fields(m, slot, input)?;
        Some((zeroed_bytes, fields))
    }

    /// Applies the bounded initializer fields plus T1743's model-0x482
    /// default-branch outputs at +0x64/+0xA8. The caller selects the default
    /// branch and supplies its loaded constants; this composition does not
    /// infer branch predicates, write ordering, or actor lifetime.
    pub fn apply_measured_initializer_fields_with_482_defaults<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        input: CharacterInitializerMeasuredInputs,
        default_sources: CharacterInitializerModel482Sources,
    ) -> Option<(
        CharacterInitializerMeasuredFields,
        CharacterInitializerModel482Outputs,
    )> {
        if input.branch.model_key != 0x0482 || self.ptrs.get(slot).copied()? == 0 {
            return None;
        }
        let fields = self.apply_measured_initializer_fields(m, slot, input)?;
        let defaults =
            self.apply_measured_initializer_model_482_default_outputs(m, slot, default_sources)?;
        Some((fields, defaults))
    }

    /// Applies the bounded initializer fields plus T1743's caller-sampled
    /// random outputs for model 0x3B3 or model 0x482/K+3=0x42. Sampling and
    /// branch selection remain caller-owned; this does not infer initializer
    /// execution order or actor lifetime.
    pub fn apply_measured_initializer_fields_with_random_outputs<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        input: CharacterInitializerMeasuredInputs,
        samples: CharacterInitializerRandomSamples,
    ) -> Option<(
        CharacterInitializerMeasuredFields,
        CharacterInitializerRandomOutputs,
    )> {
        let branch = input.branch;
        if !(branch.model_key == 0x03b3
            || (branch.model_key == 0x0482 && branch.descriptor_k3 == 0x42))
        {
            return None;
        }
        let fields = self.apply_measured_initializer_fields(m, slot, input)?;
        let random = self.apply_measured_initializer_random_outputs(
            m,
            slot,
            branch.model_key,
            branch.descriptor_k3,
            samples,
        )?;
        Some((fields, random))
    }

    /// Reads the measured aim-point triple from Rust-owned body words. The
    /// field does not establish target ownership or simulation/render timing.
    pub fn aim_point(&self, slot: usize) -> Option<CharacterAimPoint> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let offset = TsfpCharacterView::OFFSET_AIM_POINT;
        Some(CharacterAimPoint {
            coordinates: [
                f32::from_bits(self.body_word(slot, offset)),
                f32::from_bits(self.body_word(slot, offset + 4)),
                f32::from_bits(self.body_word(slot, offset + 8)),
            ],
        })
    }

    /// Updates the measured aim-point triple in owned state and mirrors its
    /// twelve bytes to every guest slot that aliases this body.
    pub fn set_aim_point_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        point: CharacterAimPoint,
    ) -> bool {
        self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_AIM_POINT,
            point.coordinates,
        )
    }

    /// Reads the inferred aim-height scalar from Rust-owned body words.
    pub fn aim_height(&self, slot: usize) -> Option<CharacterAimHeight> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterAimHeight {
            height: f32::from_bits(self.body_word(slot, TsfpCharacterView::OFFSET_AIM_HEIGHT)),
        })
    }

    /// Updates the inferred aim-height scalar and mirrors it to aliased guest
    /// slots. The writer assigns no ownership or tick semantics.
    pub fn set_aim_height_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        height: CharacterAimHeight,
    ) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_AIM_HEIGHT, height.height)
    }

    /// Reads the inferred scale scalar from Rust-owned body words.
    pub fn aim_scale(&self, slot: usize) -> Option<CharacterAimScale> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterAimScale {
            scale: f32::from_bits(self.body_word(slot, TsfpCharacterView::OFFSET_AIM_SCALE)),
        })
    }

    /// Writes the inferred scale scalar through owned state and guest memory;
    /// no aim behavior or cadence is assigned here.
    pub fn set_aim_scale_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        scale: CharacterAimScale,
    ) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_AIM_SCALE, scale.scale)
    }

    /// Reads the measured signed selector words from Rust-owned body state.
    pub fn selector_words(&self, slot: usize) -> Option<CharacterSelectorWords> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterSelectorWords {
            type_model_selector: self.body_word(slot, TsfpCharacterView::OFFSET_MODEL_SELECTOR)
                as u16 as i16,
            secondary_selector: (self.body_word(slot, TsfpCharacterView::OFFSET_MODEL_SELECTOR)
                >> 16) as u16 as i16,
        })
    }

    /// Updates each signed selector word through Rust-owned state, preserving
    /// adjacent bytes and mirroring the two writes to guest memory.
    pub fn set_selector_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        selectors: CharacterSelectorWords,
    ) -> bool {
        self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_MODEL_SELECTOR,
            selectors.type_model_selector,
        ) && self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_MODEL_SELECTOR_B,
            selectors.secondary_selector,
        )
    }

    /// Reads the measured cached-target XYZ triple from Rust-owned body words.
    pub fn cached_target_position(&self, slot: usize) -> Option<CharacterCachedTargetPosition> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        let offset = TsfpCharacterView::OFFSET_CACHED_TARGET_POSITION;
        Some(CharacterCachedTargetPosition {
            coordinates: [
                f32::from_bits(self.body_word(slot, offset)),
                f32::from_bits(self.body_word(slot, offset + 4)),
                f32::from_bits(self.body_word(slot, offset + 8)),
            ],
        })
    }

    /// Updates the cached-target XYZ triple and mirrors it through every
    /// CharacterSet slot sharing this body pointer.
    pub fn set_cached_target_position_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        position: CharacterCachedTargetPosition,
    ) -> bool {
        self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_CACHED_TARGET_POSITION,
            position.coordinates,
        )
    }

    /// Reads the measured route and goal ID words from Rust-owned body state.
    pub fn route_goal_state(&self, slot: usize) -> Option<CharacterRouteGoalState> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterRouteGoalState {
            route_id: self.body_word(slot, 0x678),
            goal_id: self.body_word(slot, 0x684),
        })
    }

    /// Updates both measured ID words and mirrors them to guest memory,
    /// preserving the intervening body words.
    pub fn set_route_goal_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        state: CharacterRouteGoalState,
    ) -> bool {
        if !self.set_word(m, slot, 0x678, state.route_id) {
            return false;
        }
        self.set_word(m, slot, 0x684, state.goal_id)
    }

    /// Reads the address-labelled words T1743 sees cleared before route query.
    pub fn route_query_preclear_words(
        &self,
        slot: usize,
    ) -> Option<CharacterRouteQueryPreclearWords> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterRouteQueryPreclearWords {
            at_9a8: self.body_word(slot, 0x9a8),
            at_9ac: self.body_word(slot, 0x9ac),
        })
    }

    /// Writes both ABI words and mirrors them to guest memory. Their additional
    /// route-query semantics remain unresolved.
    pub fn set_route_query_preclear_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterRouteQueryPreclearWords,
    ) -> bool {
        if !self.set_word(m, slot, 0x9a8, words.at_9a8) {
            return false;
        }
        self.set_word(m, slot, 0x9ac, words.at_9ac)
    }

    /// Clears both address-labelled words before the route-query path measured
    /// by T1743. The words' additional meanings remain unresolved.
    pub fn clear_route_query_preclear_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
    ) -> bool {
        self.set_route_query_preclear_words(m, slot, CharacterRouteQueryPreclearWords::default())
    }

    /// Installs caller-resolved route/goal IDs and clears the measured
    /// route-query words as one bounded setup operation. The IDs remain raw;
    /// this does not resolve them or claim route ownership or tick timing.
    pub fn prepare_route_query<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        route: CharacterRouteGoalState,
    ) -> bool {
        if self.route_goal_state(slot).is_none() {
            return false;
        }
        self.set_route_goal_state(m, slot, route) && self.clear_route_query_preclear_words(m, slot)
    }

    /// Reads the callback-record pointer word from Rust-owned body state.
    /// The raw value is not dereferenced and does not establish pointer lifetime.
    pub fn callback_record_reference(
        &self,
        slot: usize,
    ) -> Option<CharacterCallbackRecordReference> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterCallbackRecordReference {
            raw: self.body_word(slot, TsfpCharacterView::OFFSET_CALLBACK_RECORD),
        })
    }

    /// Writes the callback-record pointer word through the owned body and
    /// mirrors it to guest memory without dereferencing the pointee.
    pub fn set_callback_record_reference<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        reference: CharacterCallbackRecordReference,
    ) -> bool {
        self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_CALLBACK_RECORD,
            reference.raw,
        )
    }

    /// Reads Character+0x8 as an observed signed word. It is not interpreted as
    /// a CharacterSet slot or joined to actor identity.
    pub fn signed_index_word(&self, slot: usize) -> Option<CharacterSignedIndexWord> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterSignedIndexWord {
            raw: self.body_word(slot, 0x8) as i32,
        })
    }

    /// Writes the signed Character+0x8 word through Rust-owned body state and
    /// mirrors it to guest memory. Its index domain remains unresolved.
    pub fn set_signed_index_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        index: CharacterSignedIndexWord,
    ) -> bool {
        self.set_field(m, slot, 0x8, index.raw)
    }

    /// Reads the observed timer words from Rust-owned body state.
    pub fn timer_words(&self, slot: usize) -> Option<CharacterTimerWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterTimerWords {
            at_b30: self.body_word(slot, 0xb30),
            at_b34: self.body_word(slot, 0xb34),
        })
    }

    /// Writes both raw words through owned state and guest memory. This does
    /// not assign timer units, expiry behavior, or tick ownership.
    pub fn set_timer_words_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterTimerWords,
    ) -> bool {
        self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0)
            && self.set_word(m, slot, 0xb30, words.at_b30)
            && self.set_word(m, slot, 0xb34, words.at_b34)
    }

    /// Reads the address-labelled contact words from Rust-owned body state.
    pub fn contact_words(&self, slot: usize) -> Option<CharacterContactWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterContactWords {
            at_b68: self.body_word(slot, 0xb68),
            at_b6c: self.body_word(slot, 0xb6c),
        })
    }

    /// Writes both raw words through owned state and guest memory. This does
    /// not assign contact units, cadence, or actor-lifetime semantics.
    pub fn set_contact_words_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterContactWords,
    ) -> bool {
        self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0)
            && self.set_word(m, slot, 0xb68, words.at_b68)
            && self.set_word(m, slot, 0xb6c, words.at_b6c)
    }

    /// Reads the last tick marker at Character+0xB68 from owned state.
    pub fn contact_tick_marker(&self, slot: usize) -> Option<CharacterContactTick> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterContactTick(self.body_word(slot, 0xb68)))
    }

    /// Writes the measured current-tick marker at Character+0xB68 while
    /// preserving the adjacent +0xB6C counter. The caller supplies the tick;
    /// this method does not implement the unresolved window comparison or
    /// counter reset/re-arm behavior.
    pub fn set_contact_tick_marker<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        tick: CharacterContactTick,
    ) -> bool {
        self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0) && self.set_word(m, slot, 0xb68, tick.0)
    }

    /// Reads the low 16-bit value at Character+0xB6C without assigning units
    /// or counter lifecycle semantics to it.
    pub fn contact_counter(&self, slot: usize) -> Option<CharacterContactCounter> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterContactCounter(self.body_word(slot, 0xb6c) as u16))
    }

    /// Reads the accepted attacker counter at target Character+0xB6E.
    pub fn accepted_contact_counter(&self, slot: usize) -> Option<CharacterAcceptedContactCounter> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0)
            .then(|| CharacterAcceptedContactCounter((self.body_word(slot, 0xb6c) >> 16) as u16))
    }

    /// Applies the measured melee candidate guard: a target accepts only an
    /// attacker count greater than its stored +0xB6E count, then records that
    /// count before hit dispatch. `None` means either slot has no owned body;
    /// `Some(false)` means equal/lower was suppressed. This does not define
    /// counter meaning, reset/re-arm, callback recursion, or actor identity.
    pub fn consider_melee_contact_candidate<M: GuestMemory>(
        &mut self,
        m: &mut M,
        attacker_slot: usize,
        target_slot: usize,
    ) -> Option<bool> {
        let attacker_count = self.contact_counter(attacker_slot)?.0;
        let target_count = self.accepted_contact_counter(target_slot)?.0;
        if attacker_count <= target_count {
            return Some(false);
        }
        Some(self.set_field(m, target_slot, 0xb6e, attacker_count))
    }

    /// Applies the measured branch's conditional clear to only the low
    /// 16-bit word at Character+0xB6C. The caller supplies the branch result;
    /// this method does not decide the tick comparison or counter semantics.
    /// The upper half at +0xB6E and the tick marker at +0xB68 are preserved.
    pub fn apply_contact_counter_clear_on_branch<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        branch_clears_counter: bool,
    ) -> bool {
        if !self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0) {
            return false;
        }
        if !branch_clears_counter {
            return true;
        }
        let word = self.body_word(slot, 0xb6c) & 0xffff_0000;
        self.set_word(m, slot, 0xb6c, word)
    }

    /// Applies the measured contact-prologue writes in order: conditionally
    /// clear Character+0xB6C's low halfword, then unconditionally store the
    /// supplied tick at +0xB68. The caller supplies the branch result; this
    /// method does not choose a clock, interval comparison, wrap rule, or
    /// counter lifecycle.
    pub fn apply_contact_window_update<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        update: CharacterContactWindowUpdate,
    ) -> bool {
        if !self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0) {
            return false;
        }
        self.apply_contact_counter_clear_on_branch(m, slot, update.clear_counter)
            && self.set_contact_tick_marker(m, slot, update.tick)
    }

    /// Applies the measured contact-window update before checking one melee
    /// candidate, matching the bounded `0x160F80` instruction order. The
    /// caller supplies the branch outcome and tick. `None` means either body
    /// is absent; `Some(false)` means the candidate count was not greater;
    /// `Some(true)` records the greater count on the target before dispatch.
    /// This does not perform hit dispatch or infer counter, clock, or actor
    /// lifetime semantics.
    pub fn apply_contact_window_then_candidate<M: GuestMemory>(
        &mut self,
        m: &mut M,
        attacker_slot: usize,
        target_slot: usize,
        update: CharacterContactWindowUpdate,
    ) -> Option<bool> {
        if !self.ptrs.get(attacker_slot).is_some_and(|ptr| *ptr != 0)
            || !self.ptrs.get(target_slot).is_some_and(|ptr| *ptr != 0)
        {
            return None;
        }
        if !self.apply_contact_window_update(m, attacker_slot, update) {
            return None;
        }
        self.consider_melee_contact_candidate(m, attacker_slot, target_slot)
    }

    /// Reads the address-labelled words in T1743's inferred nav-point region.
    pub fn nav_point_words(&self, slot: usize) -> Option<CharacterNavPointWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterNavPointWords {
            at_af8: self.body_word(slot, 0xaf8),
            at_afc: self.body_word(slot, 0xafc),
            at_b00: self.body_word(slot, 0xb00),
        })
    }

    /// Writes the raw words through Rust-owned state and guest memory without
    /// assigning navigation semantics to the inferred field.
    pub fn set_nav_point_words_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterNavPointWords,
    ) -> bool {
        self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0)
            && self.set_word(m, slot, 0xaf8, words.at_af8)
            && self.set_word(m, slot, 0xafc, words.at_afc)
            && self.set_word(m, slot, 0xb00, words.at_b00)
    }

    /// Reads the inferred turn-threshold fields from Rust-owned body words.
    pub fn turn_thresholds(&self, slot: usize) -> Option<CharacterTurnThresholds> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterTurnThresholds {
            threshold: f32::from_bits(self.body_word(slot, 0xb18)),
            delta: f32::from_bits(self.body_word(slot, 0xb28)),
        })
    }

    /// Writes both inferred threshold floats through owned state and mirrors
    /// them to guest memory without assigning units or update semantics.
    pub fn set_turn_thresholds_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        thresholds: CharacterTurnThresholds,
    ) -> bool {
        self.ptrs.get(slot).is_some_and(|ptr| *ptr != 0)
            && self.set_word(m, slot, 0xb18, thresholds.threshold.to_bits())
            && self.set_word(m, slot, 0xb28, thresholds.delta.to_bits())
    }

    /// The typed record decoded from the owned words (not from guest memory). None without a body.
    pub fn owned(&mut self, slot: usize) -> Option<TsfpCharacterOwned> {
        let base = self.ptrs[slot];
        (base != 0).then(|| {
            TsfpCharacterView(base).load(&BodyMem {
                base,
                words: &mut self.bodies[slot],
            })
        })
    }

    /// Writer with write-through: sets one dword of a body (and of every slot sharing the pointer).
    /// Offset must be 4-aligned inside the body. Returns false (nothing written) for an invalid
    /// slot or a slot without a body.
    pub fn set_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        offset: u32,
        value: u32,
    ) -> bool {
        assert!(
            offset % 4 == 0 && offset < BODY_SIZE,
            "character offset {offset:#x}"
        );
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }
        for i in 0..SLOTS {
            if self.ptrs[i] == ptr {
                self.bodies[i][(offset / 4) as usize] = value;
            }
        }
        m.write32(ptr.wrapping_add(offset), value);
        true
    }

    /// Applies the measured zeroing performed immediately after allocation of
    /// the 0x118-byte actor extension. Later initializer field writers are
    /// separate calls and should run after this operation. This does not
    /// allocate storage or establish actor lifetime.
    pub fn zero_measured_initializer_prefix<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
    ) -> Option<u32> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 || INITIALIZER_ZEROED_BYTES > BODY_SIZE {
            return None;
        }
        for offset in (0..INITIALIZER_ZEROED_BYTES).step_by(4) {
            self.set_word(m, slot, offset, 0);
        }
        Some(INITIALIZER_ZEROED_BYTES)
    }

    /// Writer for any typed field of a given offset (u8, i16, f32, [f32; 3], ...): the bytes go to the
    /// owned body and to the guest in the same call. Offset plus size must stay inside the body.
    pub fn set_field<T: GuestValue, M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        offset: u32,
        v: T,
    ) -> bool {
        assert!(
            offset + T::SIZE <= BODY_SIZE,
            "character field {offset:#x}+{}",
            T::SIZE
        );
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }
        for i in 0..SLOTS {
            if self.ptrs[i] == ptr {
                v.write(
                    &mut BodyMem {
                        base: ptr,
                        words: &mut self.bodies[i],
                    },
                    ptr.wrapping_add(offset),
                );
            }
        }
        v.write(m, ptr.wrapping_add(offset));
        true
    }

    pub fn set_state<M: GuestMemory>(&mut self, m: &mut M, slot: usize, v: u32) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_STATE, v)
    }
    pub fn set_pending_state<M: GuestMemory>(&mut self, m: &mut M, slot: usize, v: i32) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_PENDING_STATE, v)
    }
    pub fn set_transition_time<M: GuestMemory>(&mut self, m: &mut M, slot: usize, v: f32) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_TRANSITION_TIME, v)
    }

    /// Writes a complete controller-entry snapshot to the owned body and
    /// guest image. Missing or empty slots return false without mutation.
    pub fn set_tick_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        state: CharacterTickState,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        let current_written = self.set_state(m, slot, state.current);
        let previous_written = self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_PREVIOUS_STATE,
            state.previous,
        );
        let time_written = self.set_transition_time(m, slot, state.transition_time);
        let pending_written = self.set_pending_state(m, slot, state.pending);
        current_written && previous_written && time_written && pending_written
    }

    /// Reads the controller's owned local-state fields. Missing or invalid
    /// slots do not produce a state snapshot.
    pub fn tick_state(&self, slot: usize) -> Option<CharacterTickState> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterTickState {
            current: self.body_word(slot, TsfpCharacterView::OFFSET_STATE),
            pending: self.body_word(slot, TsfpCharacterView::OFFSET_PENDING_STATE) as i32,
            previous: self.body_word(slot, TsfpCharacterView::OFFSET_PREVIOUS_STATE),
            transition_time: f32::from_bits(
                self.body_word(slot, TsfpCharacterView::OFFSET_TRANSITION_TIME),
            ),
        })
    }

    /// Reads the controller tuple and raw fields observed at +0x2c and +0xa90.
    /// This snapshot does not retain actor identity across ticks.
    pub fn behavior_tick_state(&self, slot: usize) -> Option<CharacterBehaviorTickState> {
        let controller = self.tick_state(slot)?;
        Some(CharacterBehaviorTickState {
            controller,
            byte_2c: self.body_word(slot, TsfpCharacterView::OFFSET_FLAGS_2C) as u8,
            word_a90: self.body_word(slot, TsfpCharacterView::OFFSET_FLAG_ACCUMULATOR),
        })
    }

    /// Writes the controller tuple and the two address-labelled fields through
    /// the owned body and guest image. Missing or empty slots
    /// are rejected before any field is changed.
    pub fn set_behavior_tick_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        state: CharacterBehaviorTickState,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        let controller_written = self.set_tick_state(m, slot, state.controller);
        let byte_written =
            self.set_field(m, slot, TsfpCharacterView::OFFSET_FLAGS_2C, state.byte_2c);
        let word_written = self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_FLAG_ACCUMULATOR,
            state.word_a90,
        );
        controller_written && byte_written && word_written
    }

    /// Reads the measured control byte at C+0x2c. This is the C
    /// extension byte from T1743, distinct from the keyed actor A buffer at
    /// A+0x2c used by `CharacterExtensionWord2C`.
    pub fn behavior_flags(&self, slot: usize) -> Option<CharacterBehaviorFlags> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterBehaviorFlags {
            raw: self.body_word(slot, TsfpCharacterView::OFFSET_FLAGS_2C) as u8,
        })
    }

    /// Writes the measured control byte while preserving neighboring bytes
    /// and all opaque bits in the supplied raw value.
    pub fn set_behavior_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        flags: CharacterBehaviorFlags,
    ) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_FLAGS_2C, flags.raw)
    }

    /// Reads the measured float at Character+0xa90 without changing the raw
    /// address-labelled contract used by `CharacterBehaviorTickState`.
    pub fn flag_controlled_accumulator(
        &self,
        slot: usize,
    ) -> Option<CharacterFlagControlledAccumulator> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterFlagControlledAccumulator {
            value: f32::from_bits(self.body_word(slot, TsfpCharacterView::OFFSET_FLAG_ACCUMULATOR)),
        })
    }

    /// Writes Character+0xa90 through the Rust-owned body and guest image.
    /// The float is stored by bit pattern; no accumulator behavior is applied.
    pub fn set_flag_controlled_accumulator<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        accumulator: CharacterFlagControlledAccumulator,
    ) -> bool {
        self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_FLAG_ACCUMULATOR,
            accumulator.value,
        )
    }

    /// Applies the measured controller-entry pending-state prologue to owned
    /// character state and writes changed fields through to the guest image.
    /// T1743 observes C+0x2c bit 2 bypassing pending application; this branch
    /// leaves the controller tuple unchanged. Otherwise, a differing
    /// nonnegative request changes current/previous/time and the pending
    /// request is consumed. Returns None for an absent slot.
    pub fn apply_pending_state_request<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
    ) -> Option<CharacterTickEffect> {
        if self.behavior_flags(slot)?.skip_pending_application() {
            return Some(CharacterTickEffect::PendingApplicationSkipped);
        }
        let state = self.tick_state(slot)?;
        let (next, effect) = state.consume_pending_request();
        if let CharacterTickEffect::StateChanged { from, to } = effect {
            debug_assert_eq!(from, state.current);
            debug_assert_eq!(to, next.current);
            self.set_state(m, slot, next.current);
            self.set_field(
                m,
                slot,
                TsfpCharacterView::OFFSET_PREVIOUS_STATE,
                next.previous,
            );
            self.set_transition_time(m, slot, next.transition_time);
        }
        self.set_pending_state(m, slot, next.pending);
        Some(effect)
    }

    pub fn set_aim_point<M: GuestMemory>(&mut self, m: &mut M, slot: usize, v: [f32; 3]) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_AIM_POINT, v)
    }
    pub fn set_melee_target<M: GuestMemory>(&mut self, m: &mut M, slot: usize, ptr: u32) -> bool {
        self.set_field(m, slot, TsfpCharacterView::OFFSET_MELEE_TARGET, ptr)
    }

    /// Reads the raw measured Object pointer word without dereferencing it or
    /// treating it as a generation-checked handle.
    pub fn object_reference(&self, slot: usize) -> Option<CharacterObjectReference> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterObjectReference {
            raw: self.body_word(slot, TsfpCharacterView::OFFSET_OBJECT),
        })
    }

    /// Writes Character+0 through Rust-owned storage and mirrors the guest
    /// image. This does not change ObjectPool membership or lifetime state.
    pub fn set_object_reference<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        reference: CharacterObjectReference,
    ) -> bool {
        self.set_word(m, slot, TsfpCharacterView::OFFSET_OBJECT, reference.raw)
    }

    /// Reads the measured signed word at Character+8. This preserves only its
    /// representation; the index domain and any link to CharacterSet slots
    /// remain unresolved.
    pub fn index_word(&self, slot: usize) -> Option<CharacterIndexWord> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterIndexWord {
            raw: self.body_word(slot, 0x8) as i32,
        })
    }

    /// Writes Character+8 through Rust-owned alias-aware body state and the
    /// guest mirror, without assigning index-domain or membership semantics.
    pub fn set_index_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        value: CharacterIndexWord,
    ) -> bool {
        self.set_field(m, slot, 0x8, value.raw)
    }

    /// Reads four pointer-like fields from the owned body without treating
    /// them as generation-checked handles. None means the slot has no body.
    pub fn target_words(&self, slot: usize) -> Option<CharacterTargetWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterTargetWords {
            melee_target: self.body_word(slot, TsfpCharacterView::OFFSET_MELEE_TARGET),
            alert_target: self.body_word(slot, TsfpCharacterView::OFFSET_ALERT_TARGET),
            threatening_object: self.body_word(slot, TsfpCharacterView::OFFSET_THREATENING_OBJECT),
            pickup_target: self.body_word(slot, TsfpCharacterView::OFFSET_PICKUP_TARGET),
        })
    }

    /// Writes the raw target/reference words into Rust-owned bodies and the
    /// guest image. Missing or out-of-range slots are unchanged.
    pub fn set_target_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterTargetWords,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        let melee = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_MELEE_TARGET,
            words.melee_target,
        );
        let alert = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_ALERT_TARGET,
            words.alert_target,
        );
        let threatening = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_THREATENING_OBJECT,
            words.threatening_object,
        );
        let pickup = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_PICKUP_TARGET,
            words.pickup_target,
        );
        melee && alert && threatening && pickup
    }

    /// Reads the four declared class-mask words. Field labels are INFERRED;
    /// this snapshot does not interpret any mask bit.
    pub fn class_mask_words(&self, slot: usize) -> Option<CharacterClassMaskWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterClassMaskWords {
            class_low: self.body_word(slot, TsfpCharacterView::OFFSET_CLASS_MASK_LO),
            class_high: self.body_word(slot, TsfpCharacterView::OFFSET_CLASS_MASK_HI),
            alternate_class_low: self.body_word(slot, TsfpCharacterView::OFFSET_ALT_CLASS_LO),
            alternate_class_high: self.body_word(slot, TsfpCharacterView::OFFSET_ALT_CLASS_HI),
        })
    }

    /// Writes the class-mask ABI words through to the guest image without
    /// assigning meanings to their bits. Missing or out-of-range slots do not
    /// mutate either copy.
    pub fn set_class_mask_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        words: CharacterClassMaskWords,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        let class_low = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_CLASS_MASK_LO,
            words.class_low,
        );
        let class_high = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_CLASS_MASK_HI,
            words.class_high,
        );
        let alternate_class_low = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_ALT_CLASS_LO,
            words.alternate_class_low,
        );
        let alternate_class_high = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_ALT_CLASS_HI,
            words.alternate_class_high,
        );
        class_low && class_high && alternate_class_low && alternate_class_high
    }

    /// Clears the four measured extension words at +0x10..+0x1c. The
    /// retirement routine clears these selected callback/state slots after
    /// active-extension unlink; this helper performs only the bounded field
    /// writes and does not own or sequence teardown.
    pub fn clear_measured_retirement_words<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
    ) -> bool {
        self.set_class_mask_words(m, slot, CharacterClassMaskWords::default())
    }

    /// Reads the inferred 0x1e-byte class-label buffer from Rust-owned body
    /// words. Bytes are kept opaque; no text encoding or terminator is assumed.
    pub fn class_label_bytes(&self, slot: usize) -> Option<CharacterClassLabelBytes> {
        let ptr = *self.ptrs.get(slot)?;
        if ptr == 0 {
            return None;
        }
        Some(CharacterClassLabelBytes {
            bytes: core::array::from_fn(|i| {
                let offset = TsfpCharacterView::OFFSET_CLASS_LABEL + i as u32;
                let word = self.body_word(slot, offset & !3);
                word.to_le_bytes()[(offset & 3) as usize]
            }),
        })
    }

    /// Writes the inferred class-label bytes through Rust-owned body state and
    /// mirrors them to guest memory. The final halfword's neighboring bytes
    /// are preserved; no string or ownership semantics are assigned.
    pub fn set_class_label_bytes<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        label: CharacterClassLabelBytes,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        for alias in 0..SLOTS {
            if self.ptrs[alias] != ptr {
                continue;
            }
            for (i, byte) in label.bytes.iter().copied().enumerate() {
                let offset = TsfpCharacterView::OFFSET_CLASS_LABEL + i as u32;
                let word_index = (offset / 4) as usize;
                let byte_index = (offset & 3) as usize;
                let mut word = self.bodies[alias][word_index].to_le_bytes();
                word[byte_index] = byte;
                self.bodies[alias][word_index] = u32::from_le_bytes(word);
            }
        }
        for (i, byte) in label.bytes.iter().copied().enumerate() {
            m.write8(
                ptr.wrapping_add(TsfpCharacterView::OFFSET_CLASS_LABEL + i as u32),
                byte,
            );
        }
        true
    }

    /// Reads the named damage/combat-history subset from Rust-owned words.
    /// The generated layout labels are inferred; stamp and ID fields remain
    /// raw values with no cross-system or lifetime semantics.
    pub fn damage_history_state(&self, slot: usize) -> Option<CharacterDamageHistoryState> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterDamageHistoryState {
            seconds_since_damage: f32::from_bits(
                self.body_word(slot, TsfpCharacterView::OFFSET_SECONDS_SINCE_DAMAGE),
            ),
            last_health_damage: f32::from_bits(
                self.body_word(slot, TsfpCharacterView::OFFSET_LAST_HEALTH_DAMAGE),
            ),
            threat_stamp: self.body_word(slot, TsfpCharacterView::OFFSET_THREAT_STAMP),
            threat_id: self.body_word(slot, TsfpCharacterView::OFFSET_THREAT_ID),
            melee_stamp: self.body_word(slot, TsfpCharacterView::OFFSET_MELEE_STAMP),
            melee_hit_count: self.body_word(slot, TsfpCharacterView::OFFSET_MELEE_HIT_COUNT) as u16,
        })
    }

    /// Writes the declared damage/combat-history fields into owned character
    /// state and mirrors their bytes for hybrid readers. Partial-width writes
    /// preserve neighboring bytes; missing or out-of-range slots are unchanged.
    pub fn set_damage_history_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        state: CharacterDamageHistoryState,
    ) -> bool {
        let Some(&ptr) = self.ptrs.get(slot) else {
            return false;
        };
        if ptr == 0 {
            return false;
        }

        let seconds = self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_SECONDS_SINCE_DAMAGE,
            state.seconds_since_damage,
        );
        let health = self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_LAST_HEALTH_DAMAGE,
            state.last_health_damage,
        );
        let threat_stamp = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_THREAT_STAMP,
            state.threat_stamp,
        );
        let threat_id = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_THREAT_ID,
            state.threat_id,
        );
        let melee_stamp = self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_MELEE_STAMP,
            state.melee_stamp,
        );
        let melee_hits = self.set_field(
            m,
            slot,
            TsfpCharacterView::OFFSET_MELEE_HIT_COUNT,
            state.melee_hit_count,
        );
        seconds && health && threat_stamp && threat_id && melee_stamp && melee_hits
    }

    /// Reads the owned goal flag words. The field names are inferred; bits are
    /// left uninterpreted for native handlers to define from measured behavior.
    pub fn goal_flags(&self, slot: usize) -> Option<CharacterFlagWords> {
        (self.ptrs[slot] != 0).then(|| CharacterFlagWords {
            low: self.body_word(slot, TsfpCharacterView::OFFSET_GOAL_FLAGS_LO),
            high: self.body_word(slot, TsfpCharacterView::OFFSET_GOAL_FLAGS_HI),
        })
    }

    /// Updates both goal flag words in the owned body and guest image.
    pub fn set_goal_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        flags: CharacterFlagWords,
    ) -> bool {
        self.set_word(m, slot, TsfpCharacterView::OFFSET_GOAL_FLAGS_LO, flags.low)
            && self.set_word(m, slot, TsfpCharacterView::OFFSET_GOAL_FLAGS_HI, flags.high)
    }

    /// Reads the owned pending flag words. The field names are inferred; bits
    /// are left uninterpreted for native handlers to define from measured behavior.
    pub fn pending_flags(&self, slot: usize) -> Option<CharacterFlagWords> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterFlagWords {
            low: self.body_word(slot, TsfpCharacterView::OFFSET_PENDING_FLAGS_LO),
            high: self.body_word(slot, TsfpCharacterView::OFFSET_PENDING_FLAGS_HI),
        })
    }

    /// Updates both pending flag words in the owned body and guest image.
    pub fn set_pending_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        flags: CharacterFlagWords,
    ) -> bool {
        self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_PENDING_FLAGS_LO,
            flags.low,
        ) && self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_PENDING_FLAGS_HI,
            flags.high,
        )
    }

    /// Applies one already-selected pending flag pair through a native Rust
    /// handler, then clears the owned pair and mirrors the clear to guest
    /// memory. The callback receives the raw words because T1660/T1743 do not
    /// assign their bit meanings here. The caller is responsible for choosing
    /// the original branch (including its default-mask path) before calling.
    pub fn apply_pending_flag_pair<M: GuestMemory, R>(
        &mut self,
        m: &mut M,
        slot: usize,
        apply: impl FnOnce(CharacterFlagWords) -> R,
    ) -> Option<R> {
        let flags = self.pending_flags(slot)?;
        let result = apply(flags);
        let cleared = self.set_pending_flags(m, slot, CharacterFlagWords::default());
        debug_assert!(cleared, "validated pending flag slot became unavailable");
        Some(result)
    }

    /// Applies either the saved pending pair or T1743's observed default pair,
    /// according to a branch decision made by the caller, then clears the
    /// saved pair through Rust-owned state and the guest-memory mirror. The
    /// evidence records the masked branch and fallback value, but does not
    /// establish the branch predicate's complete truth table or 0x17570's
    /// flag semantics.
    pub fn apply_pending_flag_pair_or_default<M: GuestMemory, R>(
        &mut self,
        m: &mut M,
        slot: usize,
        use_pending_pair: bool,
        apply: impl FnOnce(CharacterFlagWords) -> R,
    ) -> Option<R> {
        let pending = self.pending_flags(slot)?;
        let selected = if use_pending_pair {
            pending
        } else {
            CharacterFlagWords::DEFAULT_APPLICATION
        };
        let result = apply(selected);
        let cleared = self.set_pending_flags(m, slot, CharacterFlagWords::default());
        debug_assert!(cleared, "validated pending flag slot became unavailable");
        Some(result)
    }

    /// Selects the saved pair when the measured low-word mask is nonzero,
    /// otherwise selects T1743's default pair, then clears the saved words.
    /// The callback still owns the opaque 0x17570 flag semantics.
    pub fn apply_pending_flag_pair_for_masked_branch<M: GuestMemory, R>(
        &mut self,
        m: &mut M,
        slot: usize,
        apply: impl FnOnce(CharacterFlagWords) -> R,
    ) -> Option<R> {
        let pending = self.pending_flags(slot)?;
        self.apply_pending_flag_pair_or_default(
            m,
            slot,
            pending.low & PENDING_FLAGS_USE_PENDING_MASK != 0,
            apply,
        )
    }

    /// Reads the inferred selected-loadout word at Character+0x94. It is not
    /// joined to the distinct Object+0x94 model ID.
    pub fn loadout_weapon_word(&self, slot: usize) -> Option<CharacterLoadoutWeaponWord> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterLoadoutWeaponWord {
            raw: self.body_word(slot, TsfpCharacterView::OFFSET_LOADOUT_WEAPON),
        })
    }

    /// Writes the opaque Character+0x94 word through owned body state and
    /// mirrors it to guest memory for aliased CharacterSet slots.
    pub fn set_loadout_weapon_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        word: CharacterLoadoutWeaponWord,
    ) -> bool {
        self.set_word(m, slot, TsfpCharacterView::OFFSET_LOADOUT_WEAPON, word.raw)
    }

    /// Reads the opaque inferred readiness-flags word from Character+0xc6c.
    pub fn readiness_flags(&self, slot: usize) -> Option<CharacterReadinessFlags> {
        let ptr = *self.ptrs.get(slot)?;
        (ptr != 0).then(|| CharacterReadinessFlags {
            raw: self.body_word(slot, TsfpCharacterView::OFFSET_READINESS_FLAGS),
        })
    }

    /// Writes the opaque readiness-flags word through owned body state and
    /// mirrors it to guest memory and any aliased CharacterSet slots.
    pub fn set_readiness_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        flags: CharacterReadinessFlags,
    ) -> bool {
        self.set_word(
            m,
            slot,
            TsfpCharacterView::OFFSET_READINESS_FLAGS,
            flags.raw,
        )
    }

    /// Reads only the observed bit-1 gate for the selector-0x112 call. This
    /// does not assign semantics to the helper or the remaining raw bits.
    pub fn selector_0x112_gate(&self, slot: usize) -> Option<bool> {
        self.readiness_flags(slot)
            .map(CharacterReadinessFlags::selector_0x112_gate)
    }

    /// Changes the observed selector-0x112 gate bit while preserving every
    /// other owned readiness word bit and mirroring the write to aliases.
    pub fn set_selector_0x112_gate<M: GuestMemory>(
        &mut self,
        m: &mut M,
        slot: usize,
        enabled: bool,
    ) -> bool {
        let Some(flags) = self.readiness_flags(slot) else {
            return false;
        };
        self.set_readiness_flags(m, slot, flags.with_selector_0x112_gate(enabled))
    }

    /// Appends `*(object + 0x7c)` to the counted active prefix, matching REA T1660's bounded
    /// `0x15BCA0` store contract. The object and extension lifetimes remain external; this method
    /// does not create an actor identity or retain either pointer.
    pub fn append_object_extension<M: GuestMemory>(
        &mut self,
        m: &mut M,
        object: u32,
    ) -> Option<usize> {
        let len = self.active_len();
        if object == 0 || self.member_count as usize != len || len == SLOTS {
            return None;
        }
        let ptr = m.read32(object.wrapping_add(OBJECT_EXTENSION_OFFSET));
        let body = self
            .slot_of(ptr)
            .map(|slot| self.bodies[slot])
            .unwrap_or_else(|| {
                let mut words = [0; BODY_WORDS];
                if ptr != 0 {
                    for (i, word) in words.iter_mut().enumerate() {
                        *word = m.read32(ptr.wrapping_add(i as u32 * 4));
                    }
                }
                words
            });
        self.ptrs[len] = ptr;
        self.bodies[len] = body;
        self.member_count += 1;
        m.write32(TABLE + 4 * len as u32, ptr);
        m.write32(MEMBER_COUNT, self.member_count);
        Some(len)
    }

    /// Removes the first active `*(object + 0x7c)` match by moving the last active pointer into
    /// its cell and decrementing the count, matching REA T1660's bounded `0x15BCC0` contract. The
    /// guest last cell is deliberately left untouched; it is outside the new active prefix.
    pub fn remove_object_extension<M: GuestMemory>(&mut self, m: &mut M, object: u32) -> bool {
        if object == 0 {
            return false;
        }
        let ptr = m.read32(object.wrapping_add(OBJECT_EXTENSION_OFFSET));
        self.remove_extension_pointer(m, ptr).is_some()
    }

    fn remove_extension_pointer<M: GuestMemory>(&mut self, m: &mut M, ptr: u32) -> Option<usize> {
        let len = self.active_len();
        if self.member_count as usize != len || len == 0 {
            return None;
        }
        let slot = self.ptrs[..len].iter().position(|member| *member == ptr)?;
        let last = len - 1;
        if slot != last {
            self.ptrs[slot] = self.ptrs[last];
            self.bodies[slot] = self.bodies[last];
            m.write32(TABLE + 4 * slot as u32, self.ptrs[slot]);
        }
        self.ptrs[last] = 0;
        self.bodies[last] = [0; BODY_WORDS];
        self.member_count -= 1;
        m.write32(MEMBER_COUNT, self.member_count);
        Some(slot)
    }

    /// Composes active-extension unlink with clearing the owned Object+0x7C
    /// pointer. This is a caller-selected typed transition, not a claim that
    /// one measured teardown routine performs both writes. The guest list uses
    /// swap-last removal; an invalid record or inconsistent bounded count
    /// returns `None`, and a pointer absent from the active prefix returns
    /// `NotMember` without clearing it.
    /// This does not free extension storage or assign actor identity,
    /// generation, callback, or allocator lifetime semantics.
    pub fn remove_object_extension_and_clear_pointer<M: GuestMemory>(
        &mut self,
        m: &mut M,
        objects: &mut ObjectPool,
        record: usize,
    ) -> Option<CharacterRetirementMembership> {
        let extension = objects.record_extension_address(record)?.0;
        let Some(slot) = self.remove_extension_pointer(m, extension) else {
            return Some(CharacterRetirementMembership::NotMember);
        };
        let cleared =
            objects.set_record_extension_address(m, record, PoolRecordExtensionAddress(0));
        if !cleared {
            return None;
        }
        Some(CharacterRetirementMembership::Removed { slot })
    }

    /// Applies the measured reactivation Object+0x28 mask (`0x61`) before
    /// the caller-selected CharacterSet append branch. The ObjectPool record
    /// address is the attachment point to `Object+0x7C`; object/extension
    /// ownership, eligibility selection, and lifetime remain external. An
    /// append rejection is reported after the flag write, preserving the
    /// observed source order and partial transition.
    pub fn apply_object_reactivation<M: GuestMemory>(
        &mut self,
        m: &mut M,
        objects: &mut ObjectPool,
        record: usize,
        append_branch_taken: bool,
    ) -> Option<CharacterReactivationResult> {
        let object = objects.record_address(record)?;
        if !objects.apply_character_reactivation_object_flags(m, record) {
            return None;
        }
        let object_flags = objects.record_object_flags(record)?;
        let membership = if append_branch_taken {
            self.append_object_extension(m, object)
                .map(|slot| CharacterReactivationMembership::Appended { slot })
                .unwrap_or(CharacterReactivationMembership::AppendRejected)
        } else {
            CharacterReactivationMembership::BranchNotTaken
        };
        Some(CharacterReactivationResult {
            object_flags,
            membership,
        })
    }
}

/// Spawn definitions (`TsfpNpcSpawnDef`, stride 0x98, SETU header triple at +0x0C): bounded copy of
/// the placement records the level loader reads. The record size is MEASURED as the stride, only
/// 13 fields are named (INFERRED), the rest stays raw words.
pub const DEF_STRIDE: u32 = 0x98;
pub const DEF_WORDS: usize = (DEF_STRIDE / 4) as usize;
pub const DEF_CAP: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnDefs {
    pub base: u32,
    pub count: u32,
    /// Definitions beyond `DEF_CAP` are not owned (`count` keeps the guest value).
    pub defs: [[u32; DEF_WORDS]; DEF_CAP],
}

impl SpawnDefs {
    pub const EMPTY: Self = Self {
        base: 0,
        count: 0,
        defs: [[0; DEF_WORDS]; DEF_CAP],
    };

    pub fn owned_len(&self) -> usize {
        (self.count as usize).min(DEF_CAP)
    }

    /// Mirror of `count` definitions at `base` (what the SETU triple names).
    pub fn load<M: GuestMemory>(m: &M, base: u32, count: u32) -> Self {
        let mut s = Self::EMPTY;
        s.base = base;
        s.count = count;
        if base != 0 {
            for i in 0..s.owned_len() {
                for j in 0..DEF_WORDS {
                    s.defs[i][j] =
                        m.read32(base.wrapping_add(i as u32 * DEF_STRIDE + j as u32 * 4));
                }
            }
        }
        s
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        if self.base != 0 {
            for i in 0..self.owned_len() {
                for j in 0..DEF_WORDS {
                    m.write32(
                        self.base.wrapping_add(i as u32 * DEF_STRIDE + j as u32 * 4),
                        self.defs[i][j],
                    );
                }
            }
        }
    }

    /// First differing (definition index, byte offset), None when equal.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<(usize, u32)> {
        if self.base == 0 {
            return None;
        }
        for i in 0..self.owned_len() {
            for j in 0..DEF_WORDS {
                if m.read32(self.base.wrapping_add(i as u32 * DEF_STRIDE + j as u32 * 4))
                    != self.defs[i][j]
                {
                    return Some((i, j as u32 * 4));
                }
            }
        }
        None
    }

    /// Write-through store of one dword of definition `index` (out of range returns false).
    pub fn set_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        offset: u32,
        v: u32,
    ) -> bool {
        if self.base == 0 || index >= self.owned_len() || offset % 4 != 0 || offset >= DEF_STRIDE {
            return false;
        }
        self.defs[index][(offset / 4) as usize] = v;
        m.write32(
            self.base.wrapping_add(index as u32 * DEF_STRIDE + offset),
            v,
        );
        true
    }

    /// Typed byte-range write-through for one owned definition. Field meaning remains
    /// INFERRED; this only bounds the guest encoding to the declared record stride.
    pub fn set_field<T: GuestValue, M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        offset: u32,
        value: T,
    ) -> bool {
        let Some(end) = offset.checked_add(T::SIZE) else {
            return false;
        };
        if self.base == 0 || index >= self.owned_len() || end > DEF_STRIDE {
            return false;
        }

        value.write(
            &mut DefMemMut {
                words: &mut self.defs[index],
            },
            offset,
        );
        value.write(
            m,
            self.base
                .wrapping_add(index as u32 * DEF_STRIDE)
                .wrapping_add(offset),
        );
        true
    }

    /// Typed fields of definition `index` decoded from the owned words.
    pub fn owned(&self, index: usize) -> Option<super::generated_views::TsfpNpcSpawnDefOwned> {
        use super::generated_views::TsfpNpcSpawnDefView;
        (index < self.owned_len()).then(|| {
            let mut img = DefMem {
                words: self.defs[index],
            };
            TsfpNpcSpawnDefView(0).load(&mut img)
        })
    }
}

struct DefMem {
    words: [u32; DEF_WORDS],
}
impl GuestMemory for DefMem {
    fn read8(&self, a: u32) -> u8 {
        if a < DEF_STRIDE {
            self.words[(a / 4) as usize].to_le_bytes()[(a % 4) as usize]
        } else {
            0
        }
    }
    fn write8(&mut self, _a: u32, _v: u8) {}
}

struct DefMemMut<'a> {
    words: &'a mut [u32; DEF_WORDS],
}
impl GuestMemory for DefMemMut<'_> {
    fn read8(&self, a: u32) -> u8 {
        if a < DEF_STRIDE {
            self.words[(a / 4) as usize].to_le_bytes()[(a % 4) as usize]
        } else {
            0
        }
    }
    fn write8(&mut self, a: u32, v: u8) {
        if a < DEF_STRIDE {
            let word = &mut self.words[(a / 4) as usize];
            let mut bytes = word.to_le_bytes();
            bytes[(a % 4) as usize] = v;
            *word = u32::from_le_bytes(bytes);
        }
    }
}

/// T2122: where one guest store lands in the character set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharSlot {
    pub slot: usize,
    /// Byte offset of the containing dword inside the body (4-aligned).
    pub offset: u32,
}

impl CharacterSet {
    /// Decodes the dword containing `address` against the table pointers (`TABLE`), without mirroring the
    /// bodies (118 KB). None when no live body contains it (stays a raw store).
    pub fn classify<M: GuestMemory>(m: &M, address: u32) -> Option<CharSlot> {
        let a = address & !3;
        let count = (m.read32(MEMBER_COUNT) as usize).min(SLOTS);
        for slot in 0..count {
            let ptr = m.read32(TABLE + 4 * slot as u32);
            if ptr != 0 && a >= ptr && a - ptr < BODY_SIZE {
                return Some(CharSlot {
                    slot,
                    offset: a - ptr,
                });
            }
        }
        None
    }

    /// Write-through of one dword of a body (the guest word is the single copy: the transient mirror is dropped).
    pub fn write_through<M: GuestMemory>(m: &mut M, at: CharSlot, v: u32) {
        if at.slot >= (m.read32(MEMBER_COUNT) as usize).min(SLOTS) {
            return;
        }
        let ptr = m.read32(TABLE + 4 * at.slot as u32);
        m.write32(ptr.wrapping_add(at.offset), v);
    }
}
