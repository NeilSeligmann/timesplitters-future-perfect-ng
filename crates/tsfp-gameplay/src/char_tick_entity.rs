//! Message-entered entity control tick0x2A0500. No direct clip requests.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub auxiliary_flags: u8,
    pub drain: f32,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub target: u32,
    pub entity_flags: u32,
    pub entity_speed: f32,
    pub settle_timer: f32,
    pub origin: i32,
    pub object_room: i32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityLeaf {
    Awareness,
    Navigate,
    Resolve,
    Metadata,
    Classify,
    MoveAllowed,
    Status,
    CanExit,
    Reset,
    Path,
    Route,
    SetRoute(u32),
    Kind,
    Attached,
    Owner,
    Locate,
    Node,
    Steer,
}
impl EntityLeaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Awareness => 0x15E120,
            Self::Navigate => 0x15D470,
            Self::Resolve => 0x12F00,
            Self::Metadata => 0x177920,
            Self::Classify => 0x17A0A0,
            Self::MoveAllowed => 0x29FE20,
            Self::Status => 0x17A200,
            Self::CanExit => 0x177E90,
            Self::Reset => 0x177F20,
            Self::Path => 0x29FE60,
            Self::Route => 0x1596F0,
            Self::SetRoute(_) => 0x177FB0,
            Self::Kind => 0x177950,
            Self::Attached => 0x179F70,
            Self::Owner => 0x277FE0,
            Self::Locate => 0x9BCF0,
            Self::Node => 0x9A420,
            Self::Steer => 0x15F0D0,
        }
    }
}
pub trait EntityWorld {
    /// Resolve returns pointer-presence; Owner returns1 iff this character owns it;
    /// Attached returns1 iff its output object has +150 occupied, else0. Metadata
    /// syncs entity flags/speed, Node syncs room. Others return original EAX.
    fn leaf(&mut self, l: EntityLeaf, c: &mut EntityCtx) -> u32;
    /// 0x17E010(entity,object,state,speed), preserving mutable context writes.
    fn control(&mut self, c: &mut EntityCtx, speed: f32) -> i32;
    /// 0x172CD0(entity,input), x87 float result stored f32.
    fn scale_speed(&mut self, input: f32) -> f32;
    fn class(&mut self, mask: u64);
}
fn enter(c: &mut EntityCtx, s: u32) {
    if c.state != s {
        c.previous = c.state;
        c.state = s;
        c.pending = s as i32;
        c.state_time = 0.0;
    }
}
fn kind(c: &mut EntityCtx, w: &mut impl EntityWorld) -> u32 {
    w.leaf(EntityLeaf::Kind, c)
}
/// Complete fifteen-state original body1914 bytes. Entity helper internals remain leaves.
pub fn tick(c: &mut EntityCtx, w: &mut impl EntityWorld) {
    w.leaf(EntityLeaf::Awareness, c);
    if c.pending >= 0 {
        let s = c.pending as u32;
        if s != c.state {
            c.previous = c.state;
            c.state = s;
            c.state_time = 0.0;
        }
        c.pending = -1;
    }
    c.drain = if c.auxiliary_flags & 0x80 != 0 {
        c.drain + (1.0 / c.tick_rate as f32) * c.delta_ticks
    } else {
        0.0
    };
    w.leaf(EntityLeaf::Navigate, c);
    if c.state == 12 || c.state > 14 {
        return;
    }
    if c.state == 14 {
        c.origin = w.leaf(EntityLeaf::Locate, c) as i32;
        w.leaf(EntityLeaf::Node, c);
        let own = w.leaf(EntityLeaf::Owner, c) != 0;
        if !own {
            c.target = 0;
        }
        w.class(if own { 0x2000 } else { 4 });
        w.leaf(EntityLeaf::Steer, c);
        return;
    }
    if w.leaf(EntityLeaf::Resolve, c) == 0 {
        enter(c, 14);
        return;
    }
    w.leaf(EntityLeaf::Metadata, c);
    match c.state {
        0 => {
            if w.control(c, 0.0) != 2 {
                return;
            }
            match w.leaf(EntityLeaf::Classify, c) {
                0 => {
                    c.entity_speed = 8.0;
                    if w.leaf(EntityLeaf::MoveAllowed, c) == 0 || w.leaf(EntityLeaf::Status, c) > 1
                    {
                        enter(c, 1);
                    } else {
                        enter(c, 8);
                    }
                }
                1 | 2 => enter(c, 11),
                3 | 4 => enter(c, 9),
                _ => {}
            }
        }
        1 | 2 => {
            if c.state == 2 {
                c.entity_flags &= !2;
            }
            if w.control(c, 0.0) == 2 {
                enter(c, 4);
            }
        }
        3 => {
            if w.leaf(EntityLeaf::CanExit, c) != 0 {
                w.leaf(EntityLeaf::Reset, c);
                c.entity_flags |= 2;
                if w.control(c, 0.0) == 2 {
                    enter(c, 4);
                }
            } else {
                enter(c, 1);
            }
        }
        4 => {
            w.leaf(EntityLeaf::Path, c);
            let multiplier = if c.entity_flags & 1 == 0 {
                1.0
            } else {
                f32::from_bits(0xBF4CCCCD)
            };
            let speed = w.scale_speed(multiplier * c.entity_speed);
            match w.control(c, speed) {
                0 => {
                    if c.settle_timer <= 0.0 || c.entity_flags & 1 == 0 {
                        return;
                    }
                    c.settle_timer -= (1.0 / c.tick_rate as f32) * c.delta_ticks;
                    if c.settle_timer > 0.0 {
                        return;
                    }
                }
                1 => {}
                2 => {
                    enter(c, if c.entity_flags & 2 == 0 { 6 } else { 2 });
                    return;
                }
                5 => {
                    enter(c, 5);
                    c.settle_timer = 2.0;
                    return;
                }
                6 => {
                    enter(c, 3);
                    return;
                }
                10 => {
                    c.target = 0;
                    return;
                }
                _ => return,
            }
            let route = w.leaf(EntityLeaf::Route, c);
            if w.leaf(EntityLeaf::SetRoute(route), c) == 0 {
                c.entity_flags &= !1;
                return;
            }
            c.entity_flags |= 1;
            c.settle_timer = 2.0;
        }
        5 => {
            let multiplier = if c.entity_flags & 1 == 0 {
                f32::from_bits(0xBF4CCCCD)
            } else {
                f32::from_bits(0x3F4CCCCD)
            };
            let speed = w.scale_speed(multiplier * c.entity_speed);
            match w.control(c, speed) {
                2 | 5 => enter(c, 4),
                10 => c.target = 0,
                _ => {}
            }
        }
        6 => {
            if matches!(w.control(c, 1.0), 2 | 5) {
                c.entity_flags &= !3;
                let flags = kind(c, w);
                enter(c, if flags & 0x10 != 0 { 7 } else { 13 });
            }
        }
        7 => {
            w.leaf(EntityLeaf::Path, c);
            w.control(c, 0.0);
        }
        8 => {
            if w.leaf(EntityLeaf::Status, c) >= 2 {
                enter(c, 1);
            }
        }
        9 => {
            w.leaf(EntityLeaf::Path, c);
            w.control(c, 0.0);
            if c.target != 0 {
                enter(c, 10);
            } else if w.leaf(EntityLeaf::MoveAllowed, c) != 0
                && kind(c, w) & 0x10 == 0
                && w.leaf(EntityLeaf::Attached, c) == 0
            {
                enter(c, 13);
            }
        }
        10 => {
            w.leaf(EntityLeaf::Path, c);
            match w.control(c, 0.0) {
                2 | 6 | 10 => {
                    if w.leaf(EntityLeaf::Owner, c) != 0
                        && kind(c, w) & 0x10 == 0
                        && w.leaf(EntityLeaf::Attached, c) == 0
                    {
                        enter(c, 13);
                    } else {
                        enter(c, 9);
                    }
                }
                5 => {
                    if kind(c, w) & 0x10 == 0 {
                        enter(c, 13);
                    }
                }
                _ => {}
            }
        }
        11 => {
            w.leaf(EntityLeaf::Path, c);
            match w.control(c, 0.0) {
                2 | 4 => {
                    if kind(c, w) & 0x10 == 0 {
                        enter(c, 13);
                    }
                }
                10 => c.target = 0,
                _ => {}
            }
        }
        13 => {
            if w.control(c, 0.0) == 2 {
                enter(c, 14);
            }
        }
        _ => {}
    }
}
