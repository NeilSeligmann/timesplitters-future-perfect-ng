//! Scripted character sequences: SETU relocation and class tick 0x262720.
//! Evidence and unresolved engine adapters: docs/t-character-anim-selection.md section 18.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub kind: u32,
    /// Word +4; unused by the kind-11 actor teardown call.
    pub argument: u32,
    /// Word +8: node tag for kind 1, animation id for kind 8.
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceError {
    NotSetu,
    OffsetOverflow,
    ShortEntry,
    InvalidTableStride,
    TableOverflow,
    TableOutsideEntry,
    StepOverflow,
    StepsOutsideEntry,
    StepOutsideSequence,
}

impl core::fmt::Display for SequenceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::NotSetu => "not a SETU entry",
            Self::OffsetOverflow => "offset overflow",
            Self::ShortEntry => "short SETU entry",
            Self::InvalidTableStride => "sequence table stride must be 8",
            Self::TableOverflow => "sequence table overflow",
            Self::TableOutsideEntry => "sequence table outside SETU entry",
            Self::StepOverflow => "sequence step overflow",
            Self::StepsOutsideEntry => "sequence steps outside SETU entry",
            Self::StepOutsideSequence => "sequence step outside sequence",
        })
    }
}

impl std::error::Error for SequenceError {}

fn word(blob: &[u8], at: usize) -> Result<u32, SequenceError> {
    let end = at.checked_add(4).ok_or(SequenceError::OffsetOverflow)?;
    let bytes = blob.get(at..end).ok_or(SequenceError::ShortEntry)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

/// Decode the original loader's 0x6A0EB..0x6A137 relocation without mutating disc data.
/// Table +0xCC has +0xD0 entries of stride 8. Their first words are overwritten by
/// the loader; step storage starts at the offset in header +8, not at +0xD4 (stride).
pub fn parse(blob: &[u8]) -> Result<Vec<Vec<Step>>, SequenceError> {
    if blob.get(..4) != Some(b"SETU") {
        return Err(SequenceError::NotSetu);
    }
    let table = word(blob, 0xCC)? as usize;
    let count = word(blob, 0xD0)? as usize;
    if count == 0 {
        return Ok(Vec::new());
    }
    if word(blob, 0xD4)? != 8 {
        return Err(SequenceError::InvalidTableStride);
    }
    let table_end = count
        .checked_mul(8)
        .and_then(|n| table.checked_add(n))
        .ok_or(SequenceError::TableOverflow)?;
    if table_end > blob.len() {
        return Err(SequenceError::TableOutsideEntry);
    }
    let mut cursor = word(blob, 8)? as usize;
    let mut sequences = Vec::with_capacity(count);
    for index in 0..count {
        let steps = word(blob, table + 8 * index + 4)? as usize;
        let end = steps
            .checked_mul(12)
            .and_then(|n| cursor.checked_add(n))
            .ok_or(SequenceError::StepOverflow)?;
        if end > blob.len() {
            return Err(SequenceError::StepsOutsideEntry);
        }
        let sequence = (0..steps)
            .map(|i| {
                let at = cursor + i * 12;
                Ok(Step {
                    kind: word(blob, at)?,
                    argument: word(blob, at + 4)?,
                    value: word(blob, at + 8)?,
                })
            })
            .collect::<Result<Vec<_>, SequenceError>>()?;
        sequences.push(sequence);
        cursor = end;
    }
    Ok(sequences)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags_byte: u8,
    pub speed_scale: f32,
    pub step: u32,
    pub node: i32,
    pub walk_timer: u32,
    pub walk_progress: u32,
    pub flags: u32,
    pub parked_mask: u64,
    pub class_mask: u64,
    pub exit_mask: u64,
    pub end_animation: u32,
}

/// Original engine calls. Implementations must supply real engine behaviour; the
/// trait isolates selection without substituting guessed navigation or command logic.
pub trait SequenceWorld {
    /// 0x15E120(C, 1).
    fn aware(&mut self) -> bool;
    /// Record flag +0x54 & 0x20000 and valid tracked handle C+0x90. Original stores
    /// the resolved object at C+4 before 0x17570(C, 8, 0).
    fn tracked_target(&mut self) -> bool;
    fn change_class(&mut self, mask: u64);
    /// 0x9CC40(tag), with the player's C+0x678 fallback on a negative result.
    fn resolve_node(&mut self, tag: u32) -> i32;
    /// 0x15DDA0(object): releases actor resources and destroys its object.
    /// The caller must stop using the actor after this callback. No step argument is passed.
    fn despawn(&mut self);
    /// 0x59F90(object, 8, 0).
    fn animation_interrupted(&mut self) -> bool;
    fn plays(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// 0x15D500(object, C+0x684).
    fn walk_finished(&mut self, node: i32) -> bool;
    /// 0x15B1B0(O,1), 0x1594E0(O), 0x15CD00(O), optional 0x161730.
    /// Return the last predicate only; awareness gates the flag update separately.
    fn walk(&mut self) -> bool;
    /// 0xDAAE0(C) then 0x15B1B0(O,8).
    fn finish_sequence(&mut self);
}

fn enter(ctx: &mut SequenceCtx, state: u32) {
    if ctx.state != state {
        ctx.previous = ctx.state;
        ctx.state = state;
        ctx.pending = state as i32;
        ctx.state_time = 0.0;
    }
}

/// Tick the five-state sequence controller. `drain` is the original frame-dependent
/// SSE expression (image constant / tick rate * frame scale), supplied by the engine.
/// An invalid step is a data error; the original dereferences unconditionally even in state 0.
pub fn tick(
    ctx: &mut SequenceCtx,
    steps: &[Step],
    drain: f32,
    world: &mut impl SequenceWorld,
) -> Result<(), SequenceError> {
    let aware = world.aware();
    if ctx.pending >= 0 {
        let next = ctx.pending as u32;
        if next != ctx.state {
            ctx.previous = ctx.state;
            ctx.state = next;
            ctx.state_time = 0.0;
        }
        ctx.pending = -1;
    }
    ctx.speed_scale = if ctx.flags_byte & 0x80 != 0 {
        ctx.speed_scale + drain
    } else {
        0.0
    };
    let step = *steps
        .get(ctx.step as usize)
        .ok_or(SequenceError::StepOutsideSequence)?;
    if world.tracked_target() {
        world.change_class(8);
        return Ok(());
    }
    match ctx.state {
        0 => {
            enter(ctx, 1);
            ctx.step = 0;
            ctx.previous = 0;
        }
        1 => match step.kind {
            1 => {
                ctx.node = world.resolve_node(step.value);
                ctx.walk_timer = 0;
                ctx.walk_progress = 0;
                ctx.step = ctx.step.wrapping_add(1);
                enter(ctx, 3);
            }
            8 => enter(ctx, 2),
            11 => world.despawn(),
            18 => enter(ctx, 4),
            _ => {}
        },
        2 => {
            if !world.animation_interrupted() {
                if !world.plays(step.value) {
                    world.play(step.value, f32::from_bits(0x3EA8F5C3), 1.0);
                    return Ok(());
                }
                if !world.finished() {
                    return Ok(());
                }
            }
            ctx.step = ctx.step.wrapping_add(1);
            enter(ctx, 1);
        }
        3 => {
            if world.walk_finished(ctx.node) {
                enter(ctx, 1);
            } else {
                if world.walk() {
                    if aware {
                        ctx.flags = (ctx.flags & !0x40000) | 2;
                    }
                } else {
                    ctx.flags = (ctx.flags & !2) | 0x40000;
                }
                if ctx.flags & 1 != 0 {
                    ctx.parked_mask = ctx.class_mask;
                    world.change_class(0x20);
                }
            }
        }
        4 => {
            if ctx.exit_mask & 1 != 0 {
                world.finish_sequence();
                world.play(ctx.end_animation, 0.0, 1.0);
            }
            world.change_class(ctx.exit_mask);
        }
        _ => {}
    }
    Ok(())
}
