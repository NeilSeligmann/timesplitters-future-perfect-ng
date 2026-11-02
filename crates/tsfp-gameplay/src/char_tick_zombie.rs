//! T1877: the animation layer of the zombie class tick `0x2806B0` (class mask `0x40_0000_0000`, spawn records
//! `0x4010000000`, 19 on the disc). See `docs/t-character-anim-selection.md` section 13.
//!
//! The tick is a four state machine on `C[0x20]` (`C` = character extension). Each state plays one animation id with
//! `0x5F460(object, id, 0.34, speed)` unless `0x59D20` says the object already plays it (either controller slot):
//! 0 spawn / re-arm, 1 idle (`0x113` when aware of the target, else `0x112`), 2 turn / alert (`0x121`, speed 1.5),
//! 3 attack (`0x122`, then the bite with the clip time past 0.8). Timers come from the RNG `0x153B40`. Everything the engine
//! provides sits behind [`ZombieWorld`]; the attack body (ray test, damage) after the clip time test is not ported and
//! reported as [`TickEnd::Attack`]. Bit exact against the original under Unicorn (3000 worlds).

pub const ID_IDLE: u32 = 0x112;
pub const ID_IDLE_ALERT: u32 = 0x113;
pub const ID_TURN: u32 = 0x121;
pub const ID_ATTACK: u32 = 0x122;
/// Blend argument of every play (`0x3EA8F5C3`).
pub const BLEND: f32 = f32::from_bits(0x3EA8_F5C3);
/// Clip time past which the attack body runs (`[0x475CD0]`).
pub const ATTACK_TIME: f32 = 0.8;
const TIMER_STEP_IDLE: f64 = 0.05;
const TIMER_STEP_ALERT: f64 = 0.075;

/// The fields of the character extension the animation layer reads and writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZombieCtx {
    pub state: u32,
    /// `C[0x9D0]`, requested state or -1.
    pub pending: i32,
    pub previous: u32,
    /// `C[0x9D8]`, time in the state.
    pub state_time: f32,
    /// `C[0xB30]`, level clock.
    pub clock: f32,
    /// `C[0x9E0]`, deadline of the timer states.
    pub deadline: f32,
    /// `C[0x2C]` low byte; the sign bit drains `speed_scale`.
    pub flags_byte: u8,
    /// `C[0xA90]`.
    pub speed_scale: f32,
    pub target: u32,
    pub record_present: bool,
    /// `C[0xB88..0xB94]` and `C[0xA74..0xA80]`.
    pub aim_source: [u32; 3],
    pub aim: [u32; 3],
}

/// Engine services (original routines in the comments).
pub trait ZombieWorld {
    /// `0x15E120(C, 1)`: the zombie is aware of the target.
    fn aware(&mut self) -> bool;
    /// `0x153B40`: raw RNG word.
    fn rng_raw(&mut self) -> u32;
    /// `0x59D20(object, id)`: either controller slot plays the id.
    fn plays(&self, id: u32) -> bool;
    /// `0x5F460(object, id, blend, speed)`.
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    /// `0x59DD0(object[4])`: the current clip finished.
    fn clip_finished(&self) -> bool;
    /// `0x59EC0(object)`: current clip time.
    fn clip_time(&self) -> f32;
    /// `0x15A540(object, player, 0)`: the target object handle.
    fn acquire_target(&mut self) -> u32;
    /// `0x11D990(object, record[0x74], 0)`, state 0 with a spawn record.
    fn bind_record(&mut self);
    /// `0x158DB0(object, 0)`, the common per tick update (after the pending state is applied).
    fn common_update(&mut self);
    /// Epilogue test: `[[target + 0x7C] + 0x24] & 8`.
    fn target_flag8(&self, target: u32) -> bool;
    /// `0x17570(C, 0x10, 0)`: request the mode bit `0x10`.
    fn request_mode_bit(&mut self, bit: u32);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickEnd {
    Done,
    /// The bite body (ray test and damage, from `0x280A17`) would run: not ported.
    Attack,
}

fn enter(ctx: &mut ZombieCtx, next: u32) {
    ctx.previous = ctx.state;
    ctx.state = next;
    ctx.state_time = 0.0;
    ctx.pending = next as i32;
}

fn draw_deadline(ctx: &mut ZombieCtx, world: &mut impl ZombieWorld, step: f64) {
    let unit = f64::from(world.rng_raw()) * (1.0 / 4_294_967_296.0);
    ctx.deadline =
        (unit * f64::from(step as f32) + f64::from(ctx.clock) + f64::from(step as f32)) as f32;
}

/// One tick of `0x2806B0`.
pub fn tick(ctx: &mut ZombieCtx, world: &mut impl ZombieWorld) -> TickEnd {
    let aware = world.aware();
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
    world.common_update();
    match ctx.state {
        0 => {
            if ctx.record_present {
                world.bind_record();
            }
            if ctx.state != 1 {
                enter(ctx, 1);
            }
            draw_deadline(ctx, world, TIMER_STEP_IDLE);
            ctx.target = world.acquire_target();
        }
        1 => {
            if aware {
                if !world.plays(ID_IDLE_ALERT) {
                    world.play(ID_IDLE_ALERT, BLEND, 1.0);
                }
                if ctx.clock > ctx.deadline {
                    if ctx.state != 2 {
                        enter(ctx, 2);
                    }
                    draw_deadline(ctx, world, TIMER_STEP_ALERT);
                }
            } else if !world.plays(ID_IDLE) {
                world.play(ID_IDLE, BLEND, 1.0);
            }
        }
        2 => {
            if !world.plays(ID_TURN) {
                world.play(ID_TURN, BLEND, 1.5);
            } else if world.clip_finished() && ctx.clock > ctx.deadline {
                if aware {
                    if ctx.state != 3 {
                        enter(ctx, 3);
                    }
                } else if ctx.state != 0 {
                    enter(ctx, 0);
                }
            }
        }
        3 => {
            if !world.plays(ID_ATTACK) {
                world.play(ID_ATTACK, BLEND, 1.0);
                ctx.aim = ctx.aim_source;
            } else if world.clip_finished() {
                if ctx.state != 0 {
                    enter(ctx, 0);
                }
            } else if world.clip_time() > ATTACK_TIME {
                return TickEnd::Attack;
            }
        }
        _ => {}
    }
    if ctx.target != 0 && world.target_flag8(ctx.target) {
        world.request_mode_bit(0x10);
    }
    TickEnd::Done
}
