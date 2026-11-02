//! T1877: the static pose class tick `0x2648C0` (class mask `0x1_0000_0000`, 72 spawn records on the disc).
//! See `docs/t-character-anim-selection.md` section 13.
//!
//! The class holds a character on one frozen pose. State 0 reads the animation id from the spawn record word
//! `rec[0x94]` (a signed word, -1 = none), poses the clip of that id at the time `C[0xB08]`, starts the id with blend 0 and
//! speed 0 unless the object already plays it, clears the object flag `0x40` and parks. Bit exact against the original under
//! Unicorn (800 worlds).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPoseCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags_byte: u8,
    pub speed_scale: f32,
    /// `C[0xB08]`, the pose time.
    pub pose_time: f32,
    /// `rec[0x94]` of the spawn record (`None` without a record).
    pub record_pose_id: Option<i16>,
    /// Object flag word `O[0x28]`.
    pub object_flags: u32,
}

pub trait StaticPoseWorld {
    /// `0x15E120(C, 1)`, result unused by this tick.
    fn aware(&mut self);
    /// `0x59320(clip of id, 0, 0, out, time)`: pose the clip at the time.
    fn pose(&mut self, id: i32, time: f32);
    /// `0x59D20(object, id)`.
    fn plays(&self, id: i32) -> bool;
    /// `0x5F460(object, id, blend, speed)`.
    fn play(&mut self, id: i32, blend: f32, speed: f32);
    /// `0x15B1B0(object, 1)`.
    fn settle(&mut self);
}

/// One tick of `0x2648C0`.
pub fn tick(ctx: &mut StaticPoseCtx, world: &mut impl StaticPoseWorld) {
    world.aware();
    if ctx.pending >= 0 {
        let next = ctx.pending as u32;
        if next != ctx.state {
            ctx.state_time = 0.0;
            ctx.previous = ctx.state;
            ctx.state = next;
        }
        ctx.pending = -1;
    }
    ctx.speed_scale = if ctx.flags_byte & 0x80 != 0 {
        1.0f32 / 60.0 * -1.0 + ctx.speed_scale
    } else {
        0.0
    };
    if ctx.state != 0 {
        return;
    }
    let id = i32::from(ctx.record_pose_id.filter(|word| *word != -1).unwrap_or(-1));
    world.pose(id, ctx.pose_time);
    if !world.plays(id) {
        world.play(id, 0.0, 0.0);
    }
    world.settle();
    ctx.object_flags &= !0x40;
}
