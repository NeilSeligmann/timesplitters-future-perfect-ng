//! T2329: native HUD animation producers. State and tuning are owned Rust values;
//! the caller supplies simulation timing separately from display timing.

/// Weapon panel lifecycle (`0x141340`, record fields `+0x1750..+0x1764`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponAnimation {
    pub phase: u32,
    pub flash: f32,
    pub alpha: f32,
    pub slide: f32,
    pub target: f32,
    pub cycle: f32,
}

/// Original preset parameters, loaded from the original data at runtime.
#[derive(Clone, Copy, Debug)]
pub struct WeaponAnimationTuning {
    pub full: f32,
    pub fade_rate: f32,
    pub cycle_rate: f32,
    pub slide_epsilon: f32,
    pub slide_max_step: f32,
    pub slide_gain: f32,
}

/// Timing contract of the original updater. `frames` is the simulation delta;
/// rendering the same state again must not call `advance` a second time.
#[derive(Clone, Copy, Debug)]
pub struct HudTiming {
    pub rate_hz: i32,
    pub scale: f32,
    pub frames: f32,
}

/// Native attachment points of 0x142820/0x142880/0x142900/0x142990.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponPanelEvent {
    /// Explicit player-context notification (0x1427D0), preserves group cycle.
    Show,
    Selected,
    NextInGroup,
    PreviousInGroup,
    Hide,
}

impl WeaponAnimation {
    pub fn apply_event(&mut self, event: WeaponPanelEvent, full: f32) {
        match event {
            WeaponPanelEvent::Show => {}
            WeaponPanelEvent::Hide => {
                self.phase = 3;
                return;
            }
            WeaponPanelEvent::Selected => self.cycle = 0.0,
            WeaponPanelEvent::NextInGroup => self.cycle = (self.cycle + full).min(full),
            WeaponPanelEvent::PreviousInGroup => self.cycle = (self.cycle - full).max(-full),
        }
        if self.phase == 0 {
            self.slide = 0.0;
            self.target = 0.0;
        }
        self.phase = 1;
        self.flash = 0.0;
    }
    /// Weapon-switch attachment point. `cycle` is the signed group-cycle offset.
    pub fn switched(&mut self, cycle: f32) {
        self.phase = 1;
        self.flash = 0.0;
        self.cycle = cycle;
    }

    /// Native mode-1 drawer attachment: no guest target write is needed.
    pub fn set_slide_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Readable reconstruction of `0x141340`. Arithmetic order follows the lifted
    /// reference (single precision). No guest memory or register interface here.
    pub fn advance(&mut self, t: HudTiming, k: WeaponAnimationTuning, ending: bool) {
        if self.phase == 0 {
            return;
        }
        let base = k.full / t.rate_hz as f32;
        let step = base * t.scale;
        if ending {
            self.phase = 3;
        }
        if self.phase == 1 {
            self.alpha = (step * k.fade_rate + self.alpha).min(k.full);
            self.flash += step;
            if self.flash > k.full {
                self.phase = 2;
            }
        } else {
            self.alpha -= (if self.phase == 3 { k.fade_rate } else { k.full }) * step;
            if self.alpha < 0.0 {
                self.alpha = 0.0;
                self.phase = 0;
            }
        }
        if self.cycle < 0.0 {
            self.cycle = (step * k.cycle_rate + self.cycle).min(0.0);
        } else if self.cycle > 0.0 {
            self.cycle = (self.cycle - step * k.cycle_rate).max(0.0);
        }
        let difference = self.target - self.slide;
        if difference.abs() < k.slide_epsilon {
            self.slide = self.target;
        } else if self.target > self.slide {
            let amount = (difference * base * k.slide_gain).min(k.slide_max_step);
            self.slide = (t.frames * amount + self.slide).min(self.target);
        } else {
            let amount = ((self.slide - self.target) * base * k.slide_gain).min(k.slide_max_step);
            self.slide = (self.slide - t.frames * amount).max(self.target);
        }
    }
}

/// A message's lifecycle, independent of its text and renderer payload.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageAge {
    pub kind: usize,
    pub age: f32,
    pub end: f32,
    pub evict_after: f32,
    pub weight: i32,
    pub cue: i32,
    pub playing_handle: i32,
}
impl Default for MessageAge {
    fn default() -> Self {
        Self {
            kind: 0,
            age: 0.0,
            end: 0.0,
            evict_after: 0.0,
            weight: 0,
            cue: -1,
            playing_handle: -1,
        }
    }
}

/// Owned 16-entry queue. Entries are traversed from `cursor` in original ring order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MessageAges {
    pub entries: [MessageAge; 16],
    pub cursor: usize,
}

/// Sound attachment: the native audio boundary answers playback and consumes cues.
/// Implementations must preserve the original ordering of query and submission.
pub trait HudSounds {
    fn is_playing(&mut self, handle: i32) -> bool;
    fn submit(&mut self, cue: i32);
}

#[derive(Clone, Debug)]
pub struct MessagePolicy {
    pub limits: [i32; 14],
    /// Unordered conflict pairs from the original data table.
    pub conflicts: Vec<(usize, usize)>,
}

impl MessageAges {
    /// `0x14A5A0` ages, cues, admission counts and retirement; zero frame delta is a no-op (the `0x4E7944` gate).
    pub fn advance(
        &mut self,
        seconds: f32,
        frame_delta: i32,
        p: &MessagePolicy,
        audio: &mut impl HudSounds,
    ) {
        if frame_delta == 0 {
            return;
        }
        let mut total = [0i32; 14];
        let mut admitted = [0i32; 14];
        for offset in 0..16 {
            let e = &mut self.entries[(self.cursor + offset) % 16];
            if e.kind == 0 {
                continue;
            }
            assert!(e.kind < 14, "invalid message kind");
            let weight = if e.kind == 6 { 0 } else { e.weight };
            if e.age >= 0.0 {
                total[e.kind] = total[e.kind].wrapping_add(weight);
            }
            let conflict = e.kind != 6
                && (admitted[e.kind].wrapping_add(weight) > p.limits[e.kind]
                    || (1..14).any(|other| {
                        other != e.kind
                            && admitted[other] != 0
                            && (p.conflicts.contains(&(other, e.kind))
                                || p.conflicts.contains(&(e.kind, other)))
                    }));
            if !conflict {
                if e.playing_handle >= 0 && audio.is_playing(e.playing_handle) {
                    e.age = 0.0;
                } else {
                    e.age += seconds;
                    if e.age > 0.0 && e.cue >= 0 {
                        audio.submit(e.cue);
                        e.cue = -1;
                    }
                }
            } else if e.age < 0.0 {
                e.age += seconds;
                if e.age > 0.0 {
                    e.age = 0.0;
                }
            }
            if e.age >= 0.0 && e.kind != 8 {
                admitted[e.kind] = admitted[e.kind].wrapping_add(weight);
            }
        }
        for offset in 0..16 {
            let e = &mut self.entries[(self.cursor + offset) % 16];
            if e.kind == 0 {
                continue;
            }
            let weight = if e.kind == 6 { 0 } else { e.weight };
            total[e.kind] = total[e.kind].wrapping_sub(weight);
            if e.age >= e.end
                || (e.kind != 6
                    && e.age >= e.evict_after
                    && total[e.kind].wrapping_add(weight) > p.limits[e.kind])
            {
                e.kind = 0;
            }
        }
    }
}

/// The existing native message ring plus the lifecycle fields its original
/// insertion ports do not write. This is the authoritative state handed to the renderer.
pub const OWNED_MESSAGE_TRACE_BYTES: usize = 0x16e4;
pub const OWNED_MESSAGE_TRACE_WORDS: usize = OWNED_MESSAGE_TRACE_BYTES / 4;

/// Runtime values read from the original tables/globals by the kind-6 adapter.
/// Their original field meanings are not established beyond their writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kind6MessageTuning {
    pub fade_start: f32,
    pub end: f32,
    pub evict_after: f32,
    pub cue_scale: f32,
}

/// Sound request emitted after a kind-6 entry is posted (original event 0xD6C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind6CueRequest {
    pub event_id: u32,
    pub parameter: i32,
}

/// One-time ownership handoff from the legacy guest queue. The host adapter
/// decodes the measured ABI into these typed values; subsequent lifecycle
/// operations mutate only the Rust-owned queue.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageQueueSnapshot {
    pub cursor: usize,
    pub entries: [crate::messages::Message; 16],
    pub slides: [f32; 16],
    pub cues: [i32; 16],
    pub playing_handles: [i32; 16],
    pub progress: [u32; 16],
    pub input_progress: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnedMessageQueue {
    pub ring: crate::messages::MessageRing,
    pub slides: [f32; 16],
    pub cues: [Option<i32>; 16],
    pub playing_handles: [Option<i32>; 16],
    /// Per-entry input progress (original +0x164), retained by notices/clears.
    pub progress: [u32; 16],
    /// Monotonic queue-local identity source. Ring slots are borrowed storage;
    /// this generation prevents a later occupant from inheriting an old ID.
    generation_clock: u64,
    generations: [u64; 16],
}

impl OwnedMessageQueue {
    /// Rust-owned counterpart of lifted `0x148DC0` after its caller has passed
    /// the player/object eligibility checks. A zero source handle is rejected
    /// by the lifted producer; non-zero values remain opaque queue payloads.
    pub fn post_kind6(
        &mut self,
        source_handle: u32,
        age_seed: f32,
        tuning: Kind6MessageTuning,
    ) -> Option<Kind6CueRequest> {
        if source_handle == 0 {
            return None;
        }
        let slot = self.ring.cursor % self.ring.entries.len();
        self.ring.cursor = (slot + 1) % self.ring.entries.len();
        self.ring.entries[slot] = crate::messages::Message {
            kind: 6,
            text: Vec::new(),
            // The lifted producer's zero seed is stored as positive zero; for
            // non-zero seeds it stores the negated value.
            age: if age_seed == 0.0 { 0.0 } else { -age_seed },
            fade_start: tuning.fade_start,
            end: tuning.end,
            evict_after: tuning.evict_after,
            weight: 1,
            ..Default::default()
        };
        self.slides[slot] = 0.0;
        // 0x148DC0 does not write the adjacent cue/playing-handle words;
        // retain their imported slot values just like the lifted producer.
        self.progress[slot] = source_handle;
        self.replace_slot(slot);
        Some(Kind6CueRequest {
            event_id: 0x0d6c,
            parameter: (tuning.cue_scale * age_seed) as i32,
        })
    }

    /// Adopt a single decoded queue snapshot at the guest-to-Rust boundary.
    /// This preserves stale slot fields and ring position so later native
    /// insertions continue the original lifecycle exactly.
    pub fn import_snapshot(&mut self, snapshot: MessageQueueSnapshot) {
        self.ring.cursor = snapshot.cursor % self.ring.entries.len();
        self.ring.entries = snapshot.entries;
        self.slides = snapshot.slides;
        self.cues = snapshot.cues.map(Some);
        self.playing_handles = snapshot.playing_handles.map(Some);
        self.progress = snapshot.progress;
        self.generation_clock = 1;
        self.generations = [1; 16];
    }

    /// Encode the queue in the canonical original-layout trace projection.
    /// Player input progress is appended by `HudState` at the final word.
    pub fn encode_trace_window(&self, output: &mut [u32]) -> bool {
        if output.len() < OWNED_MESSAGE_TRACE_WORDS {
            return false;
        }
        let words = &mut output[..OWNED_MESSAGE_TRACE_WORDS];
        words.fill(0);
        words[0] = self.ring.cursor as u32;
        for (index, entry) in self.ring.entries.iter().enumerate() {
            let base = 2 + index * (0x16c / 4);
            words[base] = entry.kind;
            for (byte_index, byte) in entry
                .text
                .iter()
                .take(crate::messages::TEXT_BYTES)
                .take_while(|byte| **byte != 0)
                .enumerate()
            {
                words[base + 1 + byte_index / 4] |= u32::from(*byte) << ((byte_index % 4) * 8);
            }
            words[base + 0x144 / 4] = entry.age.to_bits();
            words[base + 0x148 / 4] = entry.fade_start.to_bits();
            words[base + 0x14c / 4] = self.slides[index].to_bits();
            words[base + 0x150 / 4] = entry.end.to_bits();
            words[base + 0x154 / 4] = entry.evict_after.to_bits();
            words[base + 0x158 / 4] = entry.weight;
            let fresh = self.generations[index] == 0;
            words[base + 0x15c / 4] =
                self.cues[index].map_or(if fresh { 0 } else { u32::MAX }, |cue| cue as u32);
            words[base + 0x160 / 4] = self.playing_handles[index]
                .map_or(if fresh { 0 } else { u32::MAX }, |handle| handle as u32);
            words[base + 0x164 / 4] = self.progress[index];
            words[base + 0x168 / 4] = entry.opaque;
        }
        true
    }

    fn replace_slot(&mut self, index: usize) -> MessageId {
        self.generation_clock = self
            .generation_clock
            .checked_add(1)
            .expect("message generation space exhausted");
        self.generations[index] = self.generation_clock;
        MessageId {
            slot: index,
            generation: self.generation_clock,
        }
    }

    /// Resolve a live message identity without exposing a borrowed ring pointer.
    /// Clearing or overwriting the entry makes an old ID stop resolving.
    pub fn message(&self, id: MessageId) -> Option<&crate::messages::Message> {
        let entry = self.ring.entries.get(id.slot)?;
        (self.generations[id.slot] == id.generation && entry.kind != 0).then_some(entry)
    }

    /// Reset this queue while keeping IDs from its previous contents stale even
    /// if the same ring slot is used immediately afterward.
    pub fn reset(&mut self) {
        let generation_clock = self.generation_clock;
        *self = Self {
            generation_clock,
            ..Self::default()
        };
    }

    /// Admit a decoded generic message and return a generation-qualified ID.
    pub fn admit_generic(
        &mut self,
        kind: u32,
        text: &[u8],
        delay: f32,
        preset: &GenericMessagePreset,
    ) -> MessageAdmission {
        match self.insert_generic(kind, text, delay, preset) {
            MessageInsertion::Invalid => MessageAdmission::Invalid,
            MessageInsertion::Duplicate => MessageAdmission::Duplicate,
            MessageInsertion::Inserted(slot) => MessageAdmission::Inserted(MessageId {
                slot,
                generation: self.generations[slot],
            }),
        }
    }

    /// 0x148A10's insert-then-publish operation. Cue publication happens only
    /// after admission succeeds and cannot be separated from this mutable call.
    pub fn admit_generic_with_cue(
        &mut self,
        kind: u32,
        text: &[u8],
        delay: f32,
        cue: i32,
        preset: &GenericMessagePreset,
    ) -> MessageAdmission {
        let admission = self.admit_generic(kind, text, delay, preset);
        if let MessageAdmission::Inserted(id) = admission {
            self.cues[id.slot] = Some(cue);
        }
        admission
    }

    /// T2242 display sample, one guest interval behind (see [`crate::animation_clock`]).
    /// Interpolate finite, non-decreasing ages for matching slots; fades, slides and
    /// stack shrink follow the render clock. New or rewound entries snap to `current`.
    /// Pure: no lifecycle step, eviction or cue; alpha 1 returns `current` exactly.
    pub fn display_sample(
        previous: &Self,
        current: &Self,
        sample: crate::animation_clock::AnimationSample,
    ) -> Self {
        let mut shown = current.clone();
        if sample.alpha >= 1.0 {
            return shown;
        }
        let alpha = sample.alpha.max(0.0);
        for (index, entry) in shown.ring.entries.iter_mut().enumerate() {
            let before = &previous.ring.entries[index];
            let same = entry.kind != 0
                && before.kind == entry.kind
                && before.text == entry.text
                && before.opaque == entry.opaque
                && before.age.is_finite()
                && entry.age.is_finite()
                && before.age <= entry.age
                && previous.slides[index] == current.slides[index];
            if same {
                entry.age = before.age + (entry.age - before.age) * alpha;
            }
        }
        shown
    }
}

/// Direct callers choose a notice variant rather than an original entry address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Plain,
    VariantC,
}

/// Decoded runtime preset for a generic insertion. `None` preserves a field
/// when its original branch does not write it. Text metrics and kind-table
/// admission belong to the caller's typed preset loader, not guest forwarding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GenericMessagePreset {
    pub fade_start: Option<f32>,
    pub end: Option<f32>,
    pub evict_after: Option<f32>,
    pub slide: Option<f32>,
    pub weight: Option<u32>,
    pub reset_input_progress: bool,
}

/// Read-only original-data boundary. Implementations load words from the owner's
/// runtime data; this contract never writes guest state or calls guest code.
pub trait MessagePresetData {
    fn word(&self, address: u32) -> Option<u32>;
}

/// Native text service boundary corresponding to outlined measurement and text
/// counting. Results use the original signed conversion/count contract.
pub trait MessageTextMetrics {
    fn outlined_weight(&self, text: &[u8]) -> Option<i32>;
    fn text_count(&self, text: &[u8]) -> Option<i32>;
}

/// Runtime inputs for the kind-2 pickup drawer (`0x149810`). The selected
/// font's line height and payload width are supplied by the native text
/// service; queue order and the stacking offset come from the Rust scheduler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupNoticeLayout {
    pub center_x: i32,
    pub bottom_line: i32,
    pub max_width: f32,
    pub hide_stacked: bool,
    pub alpha_scale: f32,
    pub foreground_base: u32,
    pub outline_base: u32,
}

/// Native font and measurement boundary for kind-2 pickup presentation.
pub trait PickupNoticeText {
    fn fit_pickup_font(&self, text: &[u8]) -> Option<u32>;
    fn line_top(&self, font: u32) -> Option<f32>;
    fn width(&self, font: u32, text: &[u8]) -> Option<f32>;
    fn integer(&self, value: f32) -> i32;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickupNoticeDraw {
    pub position: [i32; 2],
    pub text: Vec<u8>,
    pub font: u32,
    pub foreground: u32,
    pub outline: u32,
    /// The original still selects blend/font state for an over-width pickup,
    /// but skips its outlined text call.
    pub visible: bool,
}

/// Read-only owned kind-2 presentation. The scheduler supplies the cumulative
/// pickup offset and accounts message weight separately from this draw.
pub fn pickup_notice(
    message: &crate::messages::Message,
    offset: f32,
    layout: &PickupNoticeLayout,
    text: &impl PickupNoticeText,
) -> Option<PickupNoticeDraw> {
    if message.kind != 2 {
        return None;
    }
    let payload: Vec<u8> = message
        .text
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .collect();
    let font = text.fit_pickup_font(&payload)?;
    let line_top = text.line_top(font)?;
    let width = text.width(font, &payload)?;
    let mut alpha = if message.age > message.fade_start {
        1.0 - (message.age - message.fade_start) / (message.end - message.fade_start)
    } else {
        1.0
    };
    if layout.hide_stacked {
        let mut share = 1.0 - offset;
        if share > 1.0 {
            share = 1.0;
        } else if 0.0 > share {
            share = 0.0;
        }
        alpha *= share;
    }
    let alpha = text.integer(alpha * layout.alpha_scale) as u32;
    let x = text.integer(layout.center_x as f32 - width * 0.5);
    let y = text.integer(layout.bottom_line as f32 + line_top * offset);
    Some(PickupNoticeDraw {
        position: [x, y],
        text: payload,
        font,
        foreground: layout.foreground_base | alpha,
        outline: layout.outline_base | alpha,
        visible: layout.max_width > width,
    })
}

/// Runtime original-data tuning for the kind-8 corner notice (0x12D8E0).
/// Font selection and text rendering belong to the native text service.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerNoticeLayout {
    pub viewport_left: i32,
    pub margin_left: i32,
    pub viewport_bottom: i32,
    pub line_height: f32,
    pub height_factor: f32,
    pub baseline_offset: f32,
    pub full_alpha: f32,
    pub alpha_scale: f32,
    pub colour_base: u32,
}

/// Direct native text attachment: selects a fitting font from runtime tables
/// and converts the x87 alpha result using the text service's original contract.
pub trait CornerNoticeText {
    fn fit_corner_font(&self, text: &[u8]) -> Option<u32>;
    fn alpha_integer(&self, value: f64) -> i32;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CornerNoticeDraw {
    pub position: [i32; 2],
    pub text: Vec<u8>,
    pub font: u32,
    pub colour: u32,
}

/// Read-only kind-8 presentation. The scheduler owns admission and accounting;
/// repeated display frames neither age nor retire the supplied owned message.
pub fn corner_notice(
    message: &crate::messages::Message,
    layout: &CornerNoticeLayout,
    text: &impl CornerNoticeText,
) -> Option<CornerNoticeDraw> {
    if message.kind != 8 {
        return None;
    }
    let payload: Vec<u8> = message
        .text
        .iter()
        .copied()
        .take_while(|byte| *byte != 0)
        .collect();
    let font = text.fit_corner_font(&payload)?;
    // The original unordered comparison leaves full alpha when either age or
    // fade-start is NaN. No clamp: out-of-lifetime inputs retain that contract.
    let alpha = if message.age > message.fade_start {
        layout.full_alpha - (message.age - message.fade_start) / (message.end - message.fade_start)
    } else {
        layout.full_alpha
    };
    let y = (layout.viewport_bottom as f32 - layout.line_height * layout.height_factor)
        + layout.baseline_offset;
    let y = if y.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&y) {
        y as i32
    } else {
        i32::MIN
    };
    Some(CornerNoticeDraw {
        position: [layout.viewport_left.wrapping_add(layout.margin_left), y],
        text: payload,
        font,
        colour: layout.colour_base
            | text.alpha_integer(alpha as f64 * layout.alpha_scale as f64) as u32,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessagePresetError {
    InvalidKind,
    MissingData,
    UnknownBranch(u32),
    MissingMetrics,
}

impl GenericMessagePreset {
    /// Decode 0x12D350's runtime switch without embedding its kind mapping or
    /// tuning bytes. Text counting measures the stored payload; the long-text
    /// branch measures the complete source, matching the two original inputs.
    pub fn load_original(
        kind: u32,
        text: &[u8],
        data: &impl MessagePresetData,
        metrics: &impl MessageTextMetrics,
    ) -> Result<Self, MessagePresetError> {
        use MessagePresetError::*;
        if !(1..=13).contains(&kind) {
            return Err(InvalidKind);
        }
        let branch = data.word(0x12d678 + (kind - 1) * 4).ok_or(MissingData)?;
        let value = |address| data.word(address).map(f32::from_bits).ok_or(MissingData);
        let length = text.iter().position(|b| *b == 0).unwrap_or(text.len());
        let stored = &text[..length.min(crate::messages::TEXT_BYTES - 1)];
        let mut preset = Self::default();
        match branch {
            0x12d4a6 => {
                preset.fade_start = Some(value(0x475c78)?);
                preset.end = Some(value(0x475d10)?);
                preset.evict_after = preset.fade_start;
            }
            0x12d4d5 => {
                preset.fade_start = Some(value(0x475d20)?);
                preset.end = Some(value(0x475d00)?);
                preset.evict_after = Some(value(0x475d10)?);
                preset.weight = Some(metrics.outlined_weight(stored).ok_or(MissingMetrics)? as u32);
            }
            0x12d522 => {
                let count = metrics.text_count(stored).ok_or(MissingMetrics)?;
                let fade = count as f32 * value(0x475ce8)? + value(0x475d18)?;
                preset.fade_start = Some(fade);
                preset.end = Some(fade + value(0x475c78)?);
                preset.evict_after = preset.end;
                preset.reset_input_progress = true;
            }
            0x12d585 => {
                let extra = (length as u32).wrapping_sub(20) as i32 as f32 * value(0x47da4c)?;
                // Preserve unordered (NaN) behavior of the original comparison.
                let extra = if extra < 0.0 { 0.0 } else { extra };
                preset.fade_start = Some(extra + value(0x475d10)?);
                preset.end = preset.fade_start;
                preset.evict_after = Some(extra + value(0x475c78)?);
            }
            0x12d570 | 0x12d5e2 | 0x12d60c | 0x12d636 => {
                let (fade, slide, end) = match branch {
                    0x12d570 => (0x4788fc, None, 0x4788fc),
                    0x12d5e2 => (0x475d10, Some(0x475cd4), 0x475d18),
                    0x12d60c => (0x4785e0, Some(0x4765e0), 0x475c78),
                    _ => (0x475cd0, Some(0x475d04), 0x478860),
                };
                preset.fade_start = Some(value(fade)?);
                preset.slide = slide.map(value).transpose()?;
                preset.end = Some(value(end)?);
                preset.evict_after = preset.end;
            }
            0x12d66e => {}
            _ => return Err(UnknownBranch(branch)),
        }
        Ok(preset)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageInsertion {
    Invalid,
    Duplicate,
    Inserted(usize),
}

impl OwnedMessageQueue {
    /// Native 0x14AC80 scheduler. Callbacks receive owned payloads, pickup offsets and
    /// shared progress (never guest addresses); each admitted kind 7 consumes one action.
    /// Kind 9 contributes weight even when its presentation predicate is false.
    pub fn replay_native(
        &mut self,
        policy: &MessagePolicy,
        only_kind_9: bool,
        show_split_notice: bool,
        player_progress: &mut u32,
        mut take_action: impl FnMut() -> bool,
        mut draw: impl FnMut(&crate::messages::Message, f32, f32, u32),
    ) -> [i32; 14] {
        let mut sums = [0i32; 14];
        let mut offset = 0.0;
        for distance in 0..16 {
            let index = (self.ring.cursor % 16 + distance) % 16;
            let entry = &mut self.ring.entries[index];
            let kind = entry.kind as usize;
            if !(1..=10).contains(&kind)
                || entry
                    .age
                    .partial_cmp(&0.0)
                    .is_none_or(|o| o == std::cmp::Ordering::Less)
                || sums[kind].wrapping_add(entry.weight as i32) > policy.limits[kind]
                || (1..14).any(|other| {
                    other != kind
                        && sums[other] != 0
                        && (policy.conflicts.contains(&(other, kind))
                            || policy.conflicts.contains(&(kind, other)))
                })
                || (only_kind_9 && kind != 9)
                || kind == 6
            {
                continue;
            }
            if kind == 7 && take_action() {
                *player_progress = player_progress.wrapping_add(1);
            }
            if kind != 9 || show_split_notice {
                draw(entry, self.slides[index], offset, *player_progress);
            }
            if kind != 8 {
                sums[kind] = sums[kind].wrapping_add(entry.weight as i32);
            }
            if kind == 2 {
                let shrink_from = entry.end * crate::hud_messages::k::STACK_SHRINK;
                offset += if entry.age >= shrink_from {
                    (1.0 - (entry.age - shrink_from) / (entry.end - shrink_from))
                        * entry.weight as i32 as f32
                } else {
                    entry.weight as i32 as f32
                };
            }
            if kind == 7 && *player_progress == 3 {
                entry.kind = 0;
                *player_progress = 0;
            }
        }
        sums
    }

    /// Consume one admitted kind-7 notice (0x14AC80's progress branch).
    /// The queue scheduler must first apply age/conflict/only-kind filtering.
    /// Input is a native eligible action, independent of controller/keyboard/mouse.
    /// Draw runs before retirement, with the incremented shared player progress.
    /// Returning None leaves both queue and progress unchanged.
    pub fn consume_progress_notice<R>(
        &mut self,
        index: usize,
        player_progress: &mut u32,
        eligible_action: bool,
        draw: impl FnOnce(&crate::messages::Message, u32) -> R,
    ) -> Option<R> {
        let entry = self.ring.entries.get_mut(index)?;
        if entry.kind != 7 {
            return None;
        }
        if eligible_action {
            *player_progress = player_progress.wrapping_add(1);
        }
        let result = draw(entry, *player_progress);
        if *player_progress == 3 {
            entry.kind = 0;
            *player_progress = 0;
        }
        Some(result)
    }

    /// Shared native write contract of 0x12D350 after runtime branch decoding.
    /// Compare the complete NUL-terminated source before bounded copying: long
    /// source text must not deduplicate merely because its stored prefix matches.
    pub fn insert_generic(
        &mut self,
        kind: u32,
        text: &[u8],
        delay: f32,
        preset: &GenericMessagePreset,
    ) -> MessageInsertion {
        if !(1..=13).contains(&kind) {
            return MessageInsertion::Invalid;
        }
        let source_len = text
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(text.len());
        let source = &text[..source_len];
        if kind != 6 {
            for distance in 1..=16 {
                let index = (self.ring.cursor + 16 - distance) % 16;
                let entry = &self.ring.entries[index];
                if entry.kind == kind {
                    if entry.text == source {
                        return MessageInsertion::Duplicate;
                    }
                    break;
                }
            }
        }
        let index = self.ring.cursor;
        self.replace_slot(index);
        let entry = &mut self.ring.entries[index];
        entry.kind = kind;
        entry.text = source[..source.len().min(crate::messages::TEXT_BYTES - 1)].to_vec();
        entry.age = 0.0 - delay;
        entry.weight = preset.weight.unwrap_or(1);
        entry.opaque = u32::MAX;
        if let Some(value) = preset.fade_start {
            entry.fade_start = value;
        }
        if let Some(value) = preset.end {
            entry.end = value;
        }
        if let Some(value) = preset.evict_after {
            entry.evict_after = value;
        }
        if let Some(value) = preset.slide {
            self.slides[index] = value;
        }
        self.cues[index] = None;
        self.playing_handles[index] = None;
        self.progress[index] = 0;
        self.ring.cursor = (index + 1) % 16;
        MessageInsertion::Inserted(index)
    }

    /// Native attachments of 0x12D6B0/0x12D760. Those producers leave slide,
    /// cue and playing-handle fields untouched, including on ring wraparound.
    /// Preserve that contract instead of treating a notice as a full entry reset.
    pub fn push_notice(
        &mut self,
        kind: NoticeKind,
        text: &[u8],
        opaque: u32,
        delay: f32,
        tuning: &crate::messages::MessageTuning,
    ) {
        self.replace_slot(self.ring.cursor);
        let kind = match kind {
            NoticeKind::Plain => crate::messages::KIND_NOTICE,
            NoticeKind::VariantC => crate::messages::KIND_NOTICE_C,
        };
        self.ring.push(kind, text, opaque, delay, tuning);
    }

    /// Native 0x12D810 attachment. Clearing marks kind unused; payload, timing,
    /// cursor and sound metadata remain intact, as in the original producer.
    pub fn clear_kind(&mut self, kind: i32) {
        self.ring.clear_kind(kind);
    }

    /// Insert an already decoded message at the native ring cursor.
    pub fn push(
        &mut self,
        mut message: crate::messages::Message,
        slide: f32,
        cue: Option<i32>,
        playing_handle: Option<i32>,
    ) {
        message.text.truncate(crate::messages::TEXT_BYTES);
        let index = self.ring.cursor;
        self.replace_slot(index);
        self.ring.cursor = (index + 1) % 16;
        self.ring.entries[index] = message;
        self.slides[index] = slide;
        self.cues[index] = cue;
        self.playing_handles[index] = playing_handle;
    }

    pub fn advance(
        &mut self,
        seconds: f32,
        frames: i32,
        policy: &MessagePolicy,
        audio: &mut impl HudSounds,
    ) {
        let mut ages = MessageAges {
            cursor: self.ring.cursor,
            entries: core::array::from_fn(|i| {
                let message = &self.ring.entries[i];
                MessageAge {
                    kind: message.kind as usize,
                    age: message.age,
                    end: message.end,
                    evict_after: message.evict_after,
                    weight: message.weight as i32,
                    cue: self.cues[i].unwrap_or(-1),
                    playing_handle: self.playing_handles[i].unwrap_or(-1),
                }
            }),
        };
        ages.advance(seconds, frames, policy, audio);
        for (i, age) in ages.entries.iter().enumerate() {
            self.ring.entries[i].kind = age.kind as u32;
            self.ring.entries[i].age = age.age;
            self.cues[i] = (age.cue >= 0).then_some(age.cue);
        }
    }
}

/// Stable identity for one admitted message. The slot is meaningful only with
/// its generation; callers must resolve it through the owning queue each time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageId {
    slot: usize,
    generation: u64,
}

impl MessageId {
    pub const fn slot(self) -> usize {
        self.slot
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAdmission {
    Invalid,
    Duplicate,
    Inserted(MessageId),
}

/// Native text boundary for kind-7 reveal, font selection and wrapping.
/// Offsets account for the original runtime text encoding, not UTF-8 guesses.
pub trait ProgressNoticeText {
    fn count(&self, text: &[u8]) -> Option<u32>;
    fn prefix_offset(&self, text: &[u8], count: u32) -> Option<usize>;
    fn fit_font(&self, text: &[u8]) -> Option<u32>;
    fn wrap(&self, text: &[u8], width: f32) -> Option<Vec<u8>>;
    fn alpha_integer(&self, value: f64) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressNoticeLayout {
    pub viewport_left: i32,
    pub viewport_right: i32,
    pub viewport_bottom: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    pub margin_bottom: i32,
    pub effective_players: u32,
    pub scale: [f32; 2],
    pub reveal_rate: f32,
    pub full_alpha: f32,
    pub alpha_scale: f32,
    pub foreground_base: u32,
    pub outline_base: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgressNoticeDraw {
    pub position: [f32; 2],
    pub text: Vec<u8>,
    pub font: u32,
    pub foreground: u32,
    pub outline: u32,
}

fn notice_integer(value: f32) -> i32 {
    if value.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        value as i32
    } else {
        i32::MIN
    }
}

/// Read-only presentation of 0x12D9C0. Shared progress is supplied after the
/// scheduler's native action; two reveals all text, other values reveal by age.
/// Missing metrics or invalid encoding offsets reject the draw without mutation.
pub fn progress_notice(
    message: &crate::messages::Message,
    progress: u32,
    layout: &ProgressNoticeLayout,
    text: &impl ProgressNoticeText,
) -> Option<ProgressNoticeDraw> {
    if message.kind != 7 {
        return None;
    }
    let payload: Vec<u8> = message
        .text
        .iter()
        .copied()
        .take_while(|b| *b != 0)
        .collect();
    let count = if progress == 2 {
        text.count(&payload)?
    } else {
        notice_integer(message.age * layout.reveal_rate) as u32
    };
    // Original clamps the unsigned count against byte length before applying
    // the encoding-aware advance. Negative/NaN conversions therefore clamp.
    let count = count.min(payload.len() as u32);
    let offset = text.prefix_offset(&payload, count)?;
    let prefix = payload.get(..offset)?;
    let alpha = if message.age > message.fade_start {
        layout.full_alpha - (message.age - message.fade_start) / (message.end - message.fade_start)
    } else {
        layout.full_alpha
    };
    let alpha = text.alpha_integer(alpha as f64 * layout.alpha_scale as f64) as u32;
    // Font fitting precedes wrapping and uses the complete revealed prefix.
    let font = text.fit_font(prefix)?;
    let width = layout
        .viewport_right
        .wrapping_sub(layout.viewport_left)
        .wrapping_sub(layout.margin_right)
        .wrapping_sub(layout.margin_left)
        .wrapping_sub(40);
    let wrapped = text.wrap(prefix, width as f32)?;
    let x = layout
        .viewport_left
        .wrapping_add(layout.margin_left)
        .wrapping_add(20);
    let bottom = layout.viewport_bottom.wrapping_add(layout.margin_bottom);
    let offset = if layout.effective_players == 1 {
        80
    } else {
        40
    };
    Some(ProgressNoticeDraw {
        position: [
            notice_integer(x as f32 * layout.scale[0]) as f32,
            notice_integer(offset as f32 * layout.scale[1] + bottom as f32) as f32,
        ],
        text: wrapped,
        font,
        foreground: layout.foreground_base | alpha,
        outline: layout.outline_base | alpha,
    })
}

/// Native text boundary for the centered kind-9 notice (0x149710).
/// Centered rendering is explicit instead of the original global text flag.
pub trait SplitNoticeText {
    fn wrap(&self, font: u32, text: &[u8], width: f32) -> Option<Vec<u8>>;
    fn bottom_line(&self) -> Option<i32>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitNoticeLayout {
    pub viewport_left: i32,
    pub viewport_right: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    pub mode: u8,
    pub level_players: i32,
    pub alternate_context: bool,
    pub normal_font: u32,
    pub alternate_font: u32,
    pub width_factor: f32,
    pub foreground: u32,
    pub outline: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitNoticeDraw {
    pub position: [f32; 2],
    pub text: Vec<u8>,
    pub font: u32,
    pub foreground: u32,
    pub outline: u32,
    pub centered: bool,
}

/// Read-only kind-9 presentation. The queue scheduler owns visibility and weight
/// accounting. Missing native metrics reject the draw, without guest forwarding.
pub fn split_notice(
    message: &crate::messages::Message,
    layout: &SplitNoticeLayout,
    text: &impl SplitNoticeText,
) -> Option<SplitNoticeDraw> {
    if message.kind != 9 {
        return None;
    }
    let font = if layout.mode == 10 && layout.level_players > 1 && layout.alternate_context {
        layout.alternate_font
    } else {
        layout.normal_font
    };
    let left = layout.viewport_left.wrapping_add(layout.margin_left);
    let span = layout
        .viewport_right
        .wrapping_sub(layout.viewport_left)
        .wrapping_sub(layout.margin_left)
        .wrapping_sub(layout.margin_right);
    // The source passes this absolute-coordinate expression as wrap width.
    // Replacing it with span alone would change the original preset.
    let width = span as f32 * layout.width_factor + left as f32;
    let payload: Vec<u8> = message
        .text
        .iter()
        .copied()
        .take_while(|b| *b != 0)
        .collect();
    let wrapped = text.wrap(font, &payload, width)?;
    let center = (span >> 1).wrapping_add(left);
    let bottom = text.bottom_line()?;
    Some(SplitNoticeDraw {
        // The original kind-9 drawer passes this bottom-line value directly
        // into its centered outlined draw call (0x42130). Do not convert it to
        // a top coordinate using measured text height.
        position: [center as f32, bottom as f32],
        text: wrapped,
        font,
        foreground: layout.foreground,
        outline: layout.outline,
        centered: true,
    })
}

/// Native metric boundary for the kind-1 centered notice (0x149590).
/// Width uses the selected fitting font; bottom uses the separate player font.
pub trait CenterNoticeText {
    fn fit_font(&self, text: &[u8]) -> Option<u32>;
    fn width(&self, font: u32, text: &[u8]) -> Option<f64>;
    fn bottom_line(&self) -> Option<i32>;
    fn integer(&self, value: f64) -> i32;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CenterNoticeLayout {
    pub viewport_left: i32,
    pub viewport_right: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    pub scale_y: f32,
    pub bottom_offset: f32,
    pub line_height: f32,
    pub half_width: f32,
    pub full_alpha: f32,
    pub alpha_scale: f32,
    pub foreground_base: u32,
    pub outline_base: u32,
}

/// Read-only owned kind-1 presentation. Unlike kind 9, this drawer subtracts
/// measured half-width itself and emits a left-aligned outlined command.
pub fn center_notice(
    message: &crate::messages::Message,
    layout: &CenterNoticeLayout,
    text: &impl CenterNoticeText,
) -> Option<ProgressNoticeDraw> {
    if message.kind != 1 {
        return None;
    }
    let payload: Vec<u8> = message
        .text
        .iter()
        .copied()
        .take_while(|b| *b != 0)
        .collect();
    let lines = payload.iter().filter(|b| **b == b'\n').count() as u32 + 1;
    let alpha = if message.age > message.fade_start {
        layout.full_alpha - (message.age - message.fade_start) / (message.end - message.fade_start)
    } else {
        layout.full_alpha
    };
    let alpha = text.integer(alpha as f64 * layout.alpha_scale as f64) as u32;
    let font = text.fit_font(&payload)?;
    let span = layout
        .viewport_right
        .wrapping_sub(layout.viewport_left)
        .wrapping_sub(layout.margin_right)
        .wrapping_sub(layout.margin_left);
    let center = (span >> 1)
        .wrapping_add(layout.viewport_left)
        .wrapping_add(layout.margin_left);
    let x =
        text.integer(center as f32 as f64 - text.width(font, &payload)? * layout.half_width as f64);
    let bottom = text.bottom_line()?;
    let offset = notice_integer(layout.scale_y * layout.bottom_offset);
    let height = lines as i32 as f32 * layout.line_height;
    let y = notice_integer(bottom.wrapping_sub(offset) as f32 - height);
    Some(ProgressNoticeDraw {
        position: [x as f32, y as f32],
        text: payload,
        font,
        foreground: layout.foreground_base | alpha,
        outline: layout.outline_base | alpha,
    })
}

/// Read-only presentation commands for the queue's four legacy notice families.
/// Center/progress are left-aligned; split carries explicit centering; corner
/// uses the plain integer-position renderer rather than an outlined renderer.
#[derive(Clone, Debug, PartialEq)]
pub enum NoticeDraw {
    Outlined(ProgressNoticeDraw),
    Corner(CornerNoticeDraw),
    Split(SplitNoticeDraw),
}

pub struct NoticeLayouts {
    pub center: CenterNoticeLayout,
    pub progress: ProgressNoticeLayout,
    pub corner: CornerNoticeLayout,
    pub split: SplitNoticeLayout,
}

/// Direct presentation attachment for `replay_native`'s draw callback.
/// `Ok(None)` routes other families to their native handlers; missing metrics
/// for a supported notice are an error, never a dropped draw or legacy call.
pub fn present_notice<T>(
    message: &crate::messages::Message,
    progress: u32,
    layouts: &NoticeLayouts,
    text: &T,
) -> Result<Option<NoticeDraw>, &'static str>
where
    T: CenterNoticeText + ProgressNoticeText + CornerNoticeText + SplitNoticeText,
{
    let draw = match message.kind {
        1 => center_notice(message, &layouts.center, text).map(NoticeDraw::Outlined),
        7 => progress_notice(message, progress, &layouts.progress, text).map(NoticeDraw::Outlined),
        8 => corner_notice(message, &layouts.corner, text).map(NoticeDraw::Corner),
        9 => split_notice(message, &layouts.split, text).map(NoticeDraw::Split),
        _ => return Ok(None),
    };
    draw.map(Some).ok_or("native notice metric unavailable")
}

#[cfg(test)]
mod display_sample_tests {
    use super::{HudSounds, MessagePolicy, OwnedMessageQueue};
    use crate::animation_clock::AnimationSample;

    #[derive(Default)]
    struct NoAudio;

    impl HudSounds for NoAudio {
        fn is_playing(&mut self, _handle: i32) -> bool {
            false
        }

        fn submit(&mut self, _cue: i32) {}
    }

    #[test]
    fn kind6_owned_producer_ages_and_expires_in_typed_queue() {
        let mut queue = OwnedMessageQueue::default();
        let cue = queue
            .post_kind6(
                0x1234,
                0.0,
                super::Kind6MessageTuning {
                    fade_start: 1.0,
                    end: 2.0,
                    evict_after: 1.5,
                    cue_scale: 60.0,
                },
            )
            .expect("non-null source");
        assert_eq!(cue.event_id, 0x0d6c);
        let mut policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };
        // The recorded HUD table gives kind 6 no scheduling weight. Its entry
        // still ages and expires even when its per-kind limit is zero.
        policy.limits[6] = 0;
        let mut audio = NoAudio;

        queue.advance(1.0, 1, &policy, &mut audio);
        assert_eq!(queue.ring.entries[0].kind, 6);
        assert_eq!(queue.ring.entries[0].age, 1.0);
        queue.advance(1.0, 1, &policy, &mut audio);
        assert_eq!(queue.ring.entries[0].kind, 0);
        assert_eq!(queue.ring.entries[0].age, 2.0);
    }

    fn queue(age: f32, kind: u32, text: &[u8]) -> OwnedMessageQueue {
        let mut q = OwnedMessageQueue::default();
        let e = &mut q.ring.entries[3];
        e.kind = kind;
        e.text = text.to_vec();
        e.age = age;
        e.fade_start = 2.0;
        e.end = 3.0;
        q
    }

    fn at(alpha: f32) -> AnimationSample {
        AnimationSample {
            alpha,
            seconds: 0.0,
        }
    }

    #[test]
    fn original_alpha_one_is_current_exactly() {
        let (a, b) = (queue(2.0, 2, b"Got"), queue(2.0 + 1.0 / 60.0, 2, b"Got"));
        assert_eq!(OwnedMessageQueue::display_sample(&a, &b, at(1.0)), b);
    }

    #[test]
    fn same_message_age_interpolates_within_the_interval() {
        let (a, b) = (queue(2.0, 2, b"Got"), queue(2.5, 2, b"Got"));
        let ages: Vec<f32> = [0.0, 0.25, 0.5, 0.75]
            .iter()
            .map(|&t| {
                OwnedMessageQueue::display_sample(&a, &b, at(t))
                    .ring
                    .entries[3]
                    .age
            })
            .collect();
        assert_eq!(ages, vec![2.0, 2.125, 2.25, 2.375]);
        // Fade alpha follows: strictly decreasing across display frames.
        let alphas: Vec<f32> = [0.0, 0.5, 1.0]
            .iter()
            .map(|&t| {
                OwnedMessageQueue::display_sample(&a, &b, at(t))
                    .ring
                    .entries[3]
                    .alpha(1.0)
            })
            .collect();
        assert!(alphas[0] > alphas[1] && alphas[1] > alphas[2], "{alphas:?}");
    }

    #[test]
    fn replaced_rewound_or_cleared_entries_snap() {
        let b = queue(0.1, 2, b"Got");
        for a in [
            queue(2.9, 2, b"Old"),
            queue(0.5, 2, b"Got"),
            queue(0.0, 5, b"Got"),
            OwnedMessageQueue::default(),
        ] {
            assert_eq!(OwnedMessageQueue::display_sample(&a, &b, at(0.3)), b);
        }
        let mut slid = queue(0.0, 2, b"Got");
        slid.slides[3] = 0.5;
        assert_eq!(OwnedMessageQueue::display_sample(&slid, &b, at(0.3)), b);
    }

    #[test]
    fn untouched_fields_and_other_slots_are_current() {
        let a = queue(1.0, 2, b"Got");
        let mut b = queue(1.5, 2, b"Got");
        b.ring.cursor = 4;
        b.ring.entries[5].kind = 9;
        b.ring.entries[5].age = 0.2;
        b.progress[3] = 7;
        let shown = OwnedMessageQueue::display_sample(&a, &b, at(0.5));
        assert_eq!(shown.ring.entries[5], b.ring.entries[5]);
        assert_eq!((shown.ring.cursor, shown.progress[3]), (4, 7));
        assert_eq!(shown.ring.entries[3].age, 1.25);
    }
}
