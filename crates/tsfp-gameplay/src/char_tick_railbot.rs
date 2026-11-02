//! Readable railbot class tick, original 0x37B720. Geometry/resource leaves explicit.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    pub flags: u32,
    pub current: [f32; 3],
    pub desired: [f32; 3],
    pub velocity: [f32; 3],
    pub anchor: [f32; 2],
    pub controlled: bool,
    pub passenger: bool,
    pub request_jump: bool,
    pub player: u8,
    pub timer: f32,
    pub cooldown: f32,
    pub random_timer: f32,
    pub scale: f32,
    pub audio: f32,
    pub initial: [u32; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct RailbotCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags: u32,
    pub auxiliary_flags: u32,
    pub drain: f32,
    pub clock: f32,
    pub alert_time: f32,
    pub aware: bool,
    pub sticky_aware: bool,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub whole_ticks: i32,
    pub time_scale: f32,
    pub audio_scale: f32,
    pub motion: Option<Motion>,
    pub target: u32,
    pub previous_target: u32,
    pub model: u16,
    pub record_flags: u32,
    pub record_event: u32,
    pub callback: u32,
    pub position: [f32; 3],
    pub facing: f32,
    pub pitch: f32,
    pub turn: f32,
    pub destination: [f32; 3],
    pub floor: f32,
    pub bounds_low: f32,
    pub bounds_high: f32,
    pub height_snap: u8,
    pub target_distance_squared: f32,
    pub node: i32,
    pub route_origin: i32,
    pub player_class: u32,
    pub held: u32,
    pub pressed: u32,
    pub released: u32,
    pub effect_flags: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaf {
    Activate,
    RefreshInput,
    Jump,
    LeaveControl,
    Update,
    Move,
    Look,
    Fire,
    TriggerReady,
    TriggerReset,
    Death,
    Effect,
    StopMotion,
    Arrived,
    Navigate(i32),
    Steer,
    Walk,
    Hide,
    Event(u32),
    EventFrame(u32),
    CurrentFrame,
    TurnSound,
    Sound,
    Blend,
    Smooth,
}
impl Leaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Activate => 0x37A290,
            Self::RefreshInput => 0x37ACF0,
            Self::Jump => 0x379A30,
            Self::LeaveControl => 0x37ACB0,
            Self::Update => 0x379E30,
            Self::Move => 0x379960,
            Self::Look => 0x37B3E0,
            Self::Fire => 0x37B4A0,
            Self::TriggerReady => 0x160E80,
            Self::TriggerReset => 0x160F00,
            Self::Death => 0x27E980,
            Self::Effect => 0x11FE80,
            Self::StopMotion => 0xDAC00,
            Self::Arrived => 0x37AE20,
            Self::Navigate(_) => 0x15D500,
            Self::Steer => 0x15F0D0,
            Self::Walk => 0x15CD00,
            Self::Hide => 0xB3630,
            Self::Event(_) => 0x61630,
            Self::EventFrame(_) => 0x613D0,
            Self::CurrentFrame => 0x21760,
            Self::TurnSound => 0x21C10,
            Self::Sound => 0x468A0,
            Self::Blend => 0x5C1C0,
            Self::Smooth => 0x1515E0,
        }
    }
}
pub trait RailbotWorld {
    /// Helpers can change the context; callers reread it where original does.
    fn awareness(&mut self, ctx: &mut RailbotCtx) -> bool;
    fn leaf(&mut self, which: Leaf, ctx: &mut RailbotCtx) -> u32;
    fn button_mask(&mut self, player: u8, fire: bool) -> u32;
    fn acquire_target(&mut self, ctx: &mut RailbotCtx) -> u32;
    fn allocate(&mut self, ctx: &mut RailbotCtx) -> Motion;
    fn register_callback(&mut self, ctx: &mut RailbotCtx);
    fn plays(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn phase(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    fn rng_raw(&mut self) -> u32;
    fn wrap_angle(&mut self, value: f32) -> f32;
    /// Original0x123D0 float return, argument is motion.scale.
    fn scale_curve(&mut self, value: f32) -> f32;
    /// Original duration/current clip time query0x59CF0 and actual clip header.
    fn remaining_clip_time(&mut self) -> f32;
    fn heading(&mut self, ctx: &RailbotCtx) -> f32;
    fn locate_node(&mut self, ctx: &RailbotCtx) -> i32;
}
fn enter(c: &mut RailbotCtx, state: u32) {
    if c.state != state {
        c.previous = c.state;
        c.state = state;
        c.state_time = 0.0;
        c.pending = state as i32;
    }
}
fn random_unit(raw: u32) -> f32 {
    let signed = raw as i32;
    let mut v = signed as f32;
    if signed < 0 {
        v += 4294967296.0;
    }
    v * f32::from_bits(0x2F800000)
}
fn motion(c: &mut RailbotCtx) -> &mut Motion {
    c.motion
        .as_mut()
        .expect("railbot state requires allocated motion")
}
fn firing(c: &mut RailbotCtx, w: &mut impl RailbotWorld, period: f32) {
    motion(c).timer += (1.0 / c.tick_rate as f32) * c.whole_ticks as f32;
    while motion(c).timer > period {
        w.leaf(Leaf::Fire, c);
        motion(c).timer -= period;
    }
}
/// All thirteen states plus common preamble and angle/audio tail. A valid original
/// railbot motion record is required after startup; no synthetic absent-state fixup.
pub fn tick(c: &mut RailbotCtx, w: &mut impl RailbotWorld) {
    let mut aware = w.awareness(c);
    if c.pending >= 0 {
        let s = c.pending as u32;
        if s != c.state {
            c.state_time = 0.0;
            c.previous = c.state;
            c.state = s;
        }
        c.pending = -1;
    }
    let inv = 1.0 / c.tick_rate as f32;
    c.drain = if c.auxiliary_flags & 0x80 != 0 {
        c.drain + inv * c.delta_ticks
    } else {
        0.0
    };
    let base = inv * c.time_scale * c.delta_ticks * 90.0;
    if let Some(m) = &mut c.motion {
        m.cooldown -= inv * c.delta_ticks;
        if m.cooldown <= -0.1 {
            m.cooldown = -0.1;
        }
    }
    if c.motion.is_some() {
        if !(c.state == 4 || (c.state > 5 && c.state < 13)) {
            if motion(c).flags & 1 == 0 {
                w.leaf(Leaf::RefreshInput, c);
            } else {
                motion(c).flags &= !1;
            }
            if motion(c).request_jump {
                w.leaf(Leaf::Jump, c);
            }
        }
        if motion(c).controlled && c.state != 5 {
            c.pending = 5;
            return;
        }
        if motion(c).passenger && !motion(c).controlled && c.player_class == 0x8000 {
            let p = motion(c).player;
            let mask = w.button_mask(p, false);
            if c.released & mask == 0 {
                let mask = w.button_mask(p, false);
                if c.pressed & mask != 0 {
                    motion(c).controlled = true;
                    enter(c, 5);
                }
            } else {
                w.leaf(Leaf::LeaveControl, c);
            }
        }
    }
    c.previous_target = c.target;
    c.target = w.acquire_target(c);
    if c.state > 6 && (c.state < 9 || c.state == 10) {
        if !c.sticky_aware {
            if !aware {
                c.alert_time = 0.0;
                c.sticky_aware = false;
            } else {
                if c.alert_time < 1.0 {
                    aware = false;
                }
                c.sticky_aware = aware;
            }
        } else if !aware {
            if c.alert_time < 1.0 {
                aware = c.sticky_aware;
            }
            c.sticky_aware = aware;
        } else {
            c.alert_time = 0.0;
            c.sticky_aware = aware;
        }
    }
    c.aware = aware;
    let mut speed = base;
    match c.state {
        0 => {
            w.register_callback(c);
            c.motion = Some(w.allocate(c));
            let m = motion(c);
            m.scale = 1.0;
            m.initial = [0, 0, 0x40000000];
            m.timer = 0.0;
            m.cooldown = 0.0;

            c.callback = 0x37B3B0;
            enter(c, 1);
            if c.model == 0x620 {
                w.play(0xf7, 0.0, 0.0);
                w.leaf(Leaf::Activate, c);
            } else if c.model == 0x621 {
                if c.record_flags & 0x8000000 == 0 {
                    w.play(0xf8, 0.0, 0.0);
                    w.leaf(Leaf::Activate, c);
                } else {
                    w.leaf(Leaf::Jump, c);
                    for v in &mut motion(c).velocity {
                        *v *= 4.0;
                    }
                }
            }
        }
        1 => {
            w.leaf(Leaf::Update, c);
            w.leaf(Leaf::Move, c);
            motion(c).timer = 0.0;
            let unchanged = {
                let m = motion(c);
                m.current == m.desired
            };
            if unchanged {
                let r = random_unit(w.rng_raw());
                let v = (r * 40.0 - 50.0) + motion(c).anchor[0] - c.pitch;
                motion(c).desired[0] = w.wrap_angle(v);
                let r = random_unit(w.rng_raw());
                let v = (r * 90.0 - 45.0) + motion(c).anchor[1] - c.facing;
                motion(c).desired[1] = w.wrap_angle(v);
                motion(c).desired[2] = 0.0;
            }
            speed = base * 0.5;
            let value = motion(c).scale;
            motion(c).scale = w.scale_curve(value);
            if w.leaf(Leaf::TriggerReady, c) != 0 {
                c.clock = 0.0;
                c.alert_time = 0.0;
                enter(c, 2);
            }
        }
        2 => {
            motion(c).random_timer -= inv * c.whole_ticks as f32;
            if c.previous != 2 && c.state_time <= inv * c.whole_ticks as f32 {
                motion(c).random_timer = -1.0;
            }
            if motion(c).random_timer < 0.0 {
                let v = motion(c).scale;
                motion(c).scale = -w.scale_curve(v);
                motion(c).random_timer = (random_unit(w.rng_raw()) + 0.5) * 3.0;
            }
            w.leaf(Leaf::Update, c);
            w.leaf(Leaf::Move, c);
            w.leaf(Leaf::Look, c);
            if aware {
                c.clock = 0.0;
                if c.alert_time < 3.0 {
                    firing(c, w, 0.2);
                } else if c.alert_time > 5.0 {
                    c.alert_time = 0.0;
                }
            }
            if c.clock > 3.0 {
                enter(c, 1);
                c.clock = 0.0;
            }
            w.leaf(Leaf::TriggerReset, c);
        }
        3 => {
            if c.previous != 3 && c.state_time <= inv * c.whole_ticks as f32 {
                let v = motion(c).scale;
                motion(c).scale = w.scale_curve(v) * 0.25;
            }
            w.leaf(Leaf::Update, c);
            w.leaf(Leaf::Move, c);
            w.leaf(Leaf::Look, c);
            if c.target_distance_squared > 256.0 || c.clock > 1.5 {
                c.clock = 0.0;
                enter(c, 1);
            }
            speed = base * 0.5;
            if w.leaf(Leaf::TriggerReady, c) != 0 {
                enter(c, 2);
                c.clock = 0.0;
                c.alert_time = 0.0;
            }
        }
        4 => {
            motion(c).desired[0] = -90.0;
            if c.clock > 4.0 {
                w.leaf(Leaf::Death, c);
                return;
            }
            speed = base * 0.125;
            if c.effect_flags & 2 == 0 {
                w.leaf(Leaf::Effect, c);
            }
        }
        5 => {
            if c.player_class == 0x2000 || c.player_class == 0x8000 {
                w.leaf(Leaf::Update, c);
                let mask = w.button_mask(motion(c).player, true);
                if c.held & mask != 0 {
                    firing(c, w, 0.075);
                }
                let mask = w.button_mask(motion(c).player, false);
                speed = base * 2.0;
                if c.released & mask != 0 {
                    w.leaf(Leaf::LeaveControl, c);
                }
            }
        }
        6 => {
            let remaining = w.remaining_clip_time() / (c.time_scale * 2.5);
            motion(c).desired[0] = 0.0;
            motion(c).desired[1] = 0.0;
            motion(c).request_jump = false;
            w.leaf(Leaf::StopMotion, c);
            let height = c.position[1] - c.floor;
            speed = base * 3.0;
            if height <= 0.0 || remaining <= 0.0 {
                w.play(0x760, 0.0, 2.0);
                c.position[1] = (c.floor - c.bounds_low) + c.bounds_high;
                c.height_snap = 1;
                motion(c).timer = 0.0;
                c.clock = 0.0;
                enter(c, 7);
                c.route_origin = w.locate_node(c);
                w.leaf(Leaf::Arrived, c);
            } else {
                motion(c).velocity[1] -= inv * (height / remaining) * c.delta_ticks;
                let velocity = motion(c).velocity;
                for (p, v) in c.position.iter_mut().zip(velocity) {
                    *p += inv * c.whole_ticks as f32 * v;
                }
            }
        }
        7 => {
            let dx = c.position[0] - c.destination[0];
            let dz = c.position[2] - c.destination[2];
            let d = dx * dx + dz * dz;
            if !aware {
                motion(c).desired[0] = 0.0;
                motion(c).desired[1] = 0.0;
            } else {
                w.leaf(Leaf::Look, c);
            }
            if !w.plays(0x760) {
                w.play(0x760, f32::from_bits(0x3EAA7EFA), 2.0);
            }
            if c.flags & 1 != 0 || d.abs() < 0.1 {
                w.leaf(Leaf::Arrived, c);
            }
            w.leaf(Leaf::Navigate(c.node), c);
            w.leaf(Leaf::Steer, c);
            speed = base * 3.0;
            if !aware || (d.abs() >= 0.1 && c.clock <= 6.0) || c.target_distance_squared >= 100.0 {
                if w.heading(c).abs() < 20.0 {
                    w.leaf(Leaf::Walk, c);
                } else {
                    motion(c).flags &= !2;
                    w.play(0x767, f32::from_bits(0x3EAA7EFA), 1.0);
                    enter(c, 9);
                }
            } else {
                w.play(0x769, f32::from_bits(0x3EA8F5C3), 1.0);
                enter(c, 10);
            }
        }
        8 | 9 => {
            let state = c.state;
            if !aware {
                motion(c).desired[0] = 0.0;
                motion(c).desired[1] = 0.0;
            } else {
                w.leaf(Leaf::Look, c);
            }
            let id = if state == 8 { 0x768 } else { 0x767 };
            speed = base * 3.0;
            if w.transition(id) {
                w.play(0x760, 0.0, 2.0);
                enter(c, 7);
            } else if w.plays(id) {
                if state == 8 {
                    w.leaf(Leaf::Steer, c);
                } else if w.phase(id)
                    && w.leaf(Leaf::Event(0x200), c) != 0
                    && w.leaf(Leaf::Event(4), c) == 0
                {
                    let angle = w.heading(c);
                    let current = w.leaf(Leaf::CurrentFrame, c);
                    let event = w.leaf(Leaf::EventFrame(4), c);
                    let frames = event.wrapping_sub(current) as i32;
                    if frames > 0 {
                        c.facing += angle / frames as f32;
                        c.turn = c.facing;
                    }
                    if motion(c).flags & 2 == 0 {
                        w.leaf(Leaf::TurnSound, c);
                        motion(c).flags |= 2;
                    }
                }
            } else {
                enter(c, 7);
            }
        }
        10 => {
            w.leaf(Leaf::Look, c);
            speed = base * 3.0;
            if !w.plays(0x769) {
                w.play(0x769, f32::from_bits(0x3EA8F5C3), 1.0);
            }
            firing(c, w, 0.2);
            if c.state_time > 2.0
                && (!aware
                    || c.target_distance_squared > f32::from_bits(0x42F00001)
                    || c.state_time > 5.0)
            {
                enter(c, 7);
                c.clock = 0.0;
                w.leaf(Leaf::Arrived, c);
            }
        }
        11 => {
            motion(c).desired[0] = 0.0;
            motion(c).desired[1] = 0.0;
            speed = base * 3.0;
            if w.finished() {
                w.leaf(Leaf::Hide, c);
                enter(c, 12);
            }
        }
        12 => {
            c.auxiliary_flags |= 0x100000;
        }
        _ => {}
    }
    let m = motion(c);
    if m.controlled {
        m.current = m.desired;
    }
    for n in 0..3 {
        let mut delta = m.desired[n] - m.current[n];
        while delta <= -180.0 {
            delta += 360.0;
        }
        while delta > 180.0 {
            delta -= 360.0;
        }
        let max = if n == 1 && !m.controlled {
            speed * f32::from_bits(0x3EAAAAAB)
        } else {
            speed
        };
        if delta > max {
            m.current[n] += max;
        } else if delta < -max {
            m.current[n] -= max;
        } else {
            m.current[n] = m.desired[n];
        }
    }
    w.leaf(Leaf::Smooth, c);
    w.leaf(Leaf::Sound, c);
    c.flags &= !0x60000;
    w.leaf(Leaf::Blend, c);
}
