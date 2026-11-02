//! Cover-class state controller, original 0x2A4120. Readable numeric state/id contract.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverNode {
    pub tag: i16,
    pub flags: u32,
    pub position: [f32; 3],
    pub yaw: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CoverCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub flags: u32,
    pub auxiliary_flags: u32,
    pub drain: f32,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub clock: f32,
    pub unseen: f32,
    pub action_deadline: f32,
    pub action_timer: f32,
    pub selected: bool,
    pub target: u32,
    pub target_node: i32,
    pub callback_present: bool,
    pub reservation: Option<CoverNode>,
    pub saved_reservation: Option<CoverNode>,
    pub node_flags: u32,
    pub node: i32,
    pub walk_timer: u32,
    pub walk_progress: u32,
    pub player_node: i32,
    pub record_mask: u64,
    pub origin: i32,
    pub destination: [f32; 3],
    pub position: [f32; 3],
    pub eligible: bool,
    pub posture: i32,
    pub weapon_flags: u32,
    pub assessment: [f32; 3],
    pub clip_policy: u32,
    pub slot_flags: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CoverLeaf {
    Navigate,
    Alert,
    Callback,
    Assess(bool),
    BreakCover,
    Eligible,
    Posture(u32, bool),
    Decision,
    AmbushDecision,
    CoverAction(f32),
    FinishAction,
    Align(f32),
    Face,
    Blend(u32),
    Walk,
    Fire,
    TurnClip,
    ExitClip,
    Unreserve(i16),
    ExitMotion,
    MarkNode(i32),
}
impl CoverLeaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Navigate => 0x15D470,
            Self::Alert => 0x267FF0,
            Self::Callback => 0,
            Self::Assess(_) => 0x2A14B0,
            Self::BreakCover => 0x2A16D0,
            Self::Eligible => 0x2A0ED0,
            Self::Posture(_, _) => 0x2A17B0,
            Self::Decision => 0x2A2590,
            Self::AmbushDecision => 0x2A2290,
            Self::CoverAction(_) => 0x2A0D10,
            Self::FinishAction => 0x2A0E20,
            Self::Align(_) => 0x266250,
            Self::Face => 0x159300,
            Self::Blend(_) => 0x5C1C0,
            Self::Walk => 0x15CD00,
            Self::Fire => 0x161D40,
            Self::TurnClip => 0x2A1910,
            Self::ExitClip => 0x2661F0,
            Self::Unreserve(_) => 0x9CCA0,
            Self::ExitMotion => 0x2B02A0,
            Self::MarkNode(_) => 0x9A420,
        }
    }
}
pub trait CoverWorld {
    fn awareness(&mut self, c: &mut CoverCtx) -> bool;
    /// Mutable context preserves helper writes. Return is EAX (float helpers below).
    fn leaf(&mut self, l: CoverLeaf, c: &mut CoverCtx) -> u32;
    fn find_cover(&mut self, c: &mut CoverCtx, from_record: bool) -> Option<CoverNode>;
    fn resolve_node(&mut self, tag: i16) -> i32;
    fn target_angle(&mut self, c: &CoverCtx) -> f32;
    fn wants_exit(&mut self, c: &mut CoverCtx) -> bool;
    fn attack_ready(&mut self, c: &mut CoverCtx, aware: bool) -> bool;
    fn plays(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn event_ready(&mut self, bit: u32) -> bool;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    fn class(&mut self, mask: u64);
    fn rng_raw(&mut self) -> u32;
}
const IDS: [[u32; 2]; 17] = [
    [0x5ef, 0x5db],
    [0x5dc, 0x5c8],
    [0x5e1, 0x5cd],
    [0x5de, 0x5ca],
    [0x5df, 0x5cb],
    [0x5e0, 0x5cc],
    [0x5f8, 0x5f7],
    [0x5dd, 0x5c9],
    [0x5e9, 0x5d5],
    [0x5e8, 0x5d4],
    [0x5e6, 0x5d2],
    [0x5e3, 0x5cf],
    [0x5e4, 0x5d0],
    [0x5e5, 0x5d1],
    [0x5f6, 0x5f5],
    [0x5e2, 0x5ce],
    [0x5e7, 0x5d3],
];
fn id(c: &CoverCtx, n: usize) -> u32 {
    IDS[n][usize::from(c.posture != 1)]
}
pub(crate) fn enter(c: &mut CoverCtx, state: u32) {
    if c.state != state {
        c.previous = c.state;
        c.state = state;
        c.pending = state as i32;
        c.state_time = 0.0;
    }
}
pub(crate) fn unit(raw: u32) -> f64 {
    f64::from(raw) / 4294967296.0
}

pub(crate) fn common(c: &mut CoverCtx, w: &mut impl CoverWorld, align: Option<f32>, blend: bool) {
    if let Some(a) = align {
        w.leaf(CoverLeaf::Align(a), c);
    }
    w.leaf(CoverLeaf::Face, c);
    if blend {
        c.flags &= !0x60000;
        w.leaf(CoverLeaf::Blend(1), c);
    }
}
fn sustain(c: &mut CoverCtx, w: &mut impl CoverWorld, aware: bool, high: bool) {
    let (entry, idle, attack, alternate, exit) = if high {
        (10, 11, 12, 13, 15)
    } else {
        (2, 3, 4, 5, 7)
    };
    if ![entry, idle, attack, alternate, exit]
        .into_iter()
        .any(|n| w.plays(id(c, n)))
    {
        w.play(id(c, idle), f32::from_bits(0x3EAA7EFA), 1.0);
    }
    let mut exit_state = false;
    if w.transition(id(c, entry)) {
        w.play(id(c, idle), 0.0, 1.0);
    } else if w.transition(id(c, attack)) || w.transition(id(c, alternate)) {
        if !w.plays(id(c, exit)) {
            w.play(id(c, idle), 0.0, 1.0);
        }
    } else if w.transition(id(c, exit)) {
        w.play(id(c, if high { 9 } else { 0 }), 0.0, 1.0);
        c.auxiliary_flags &= !0x400;
        exit_state = true;
    }
    if exit_state {
        enter(c, 2);
        return;
    }
    if !w.plays(id(c, exit)) {
        if [idle, attack, alternate]
            .into_iter()
            .any(|n| w.plays(id(c, n)))
        {
            c.auxiliary_flags |= 0x400;
            if !c.selected {
                w.leaf(CoverLeaf::Assess(false), c);
            }
            let active = if !aware {
                c.unseen < 2.0
            } else {
                c.unseen = 0.0;
                true
            };
            if !w.wants_exit(c) {
                if w.attack_ready(c, active) {
                    let armed = c.weapon_flags & 1 != 0;
                    w.play(id(c, if armed { attack } else { alternate }), 0.0, 1.0);
                    let offset = unit(w.rng_raw()) * 0.5
                        + if armed {
                            f64::from(f32::from_bits(0x3E99999A))
                        } else {
                            1.5
                        };
                    c.action_deadline = (offset + f64::from(c.clock)) as f32;
                }
            } else {
                let blend = if !w.plays(id(c, attack)) && !w.plays(id(c, alternate)) {
                    0.0
                } else {
                    f32::from_bits(0x3EAA7EFA)
                };
                w.play(id(c, exit), blend, 1.0);
            }
        }
    } else {
        w.leaf(CoverLeaf::Align(0.01), c);
    }
    if w.plays(id(c, entry)) {
        w.leaf(CoverLeaf::CoverAction(if high { 12.0 } else { 0.0 }), c);
        return;
    }
    if w.plays(id(c, exit)) {
        w.leaf(CoverLeaf::FinishAction, c);
    }
}
/// All13 states and common setup. Reserved node/input/motion helper closure is explicit.
pub fn tick(c: &mut CoverCtx, w: &mut impl CoverWorld) {
    let aware = w.awareness(c);
    if c.pending >= 0 {
        let s = c.pending as u32;
        if s != c.state {
            c.state_time = 0.0;
            c.previous = c.state;
            c.state = s;
        }
        c.pending = -1;
    }
    c.drain = if c.auxiliary_flags & 0x80 != 0 {
        c.drain + (1.0 / c.tick_rate as f32) * c.delta_ticks
    } else {
        0.0
    };
    w.leaf(CoverLeaf::Navigate, c);
    if c.selected && c.reservation.is_some() {
        if aware {
            w.leaf(CoverLeaf::Alert, c);
        }
        if c.selected && c.callback_present {
            let target = w.leaf(CoverLeaf::Callback, c);
            if target != 0 {
                c.target = target;
            }
        }
        if c.target == 0 {
            w.class(0x200);
            return;
        }
        w.leaf(CoverLeaf::Assess(true), c);
        if w.leaf(CoverLeaf::BreakCover, c) != 0 {
            enter(c, 12);
        }
    }
    match c.state {
        0 => {
            if c.record_mask & 0x40000 != 0 && c.reservation.is_none() {
                c.reservation = w.find_cover(c, c.record_mask & 0x800000 == 0);
                let Some(node) = c.reservation.clone() else {
                    w.class(4);
                    return;
                };
                w.resolve_node(node.tag);
                c.node_flags |= 0x40000000;
                c.destination = node.position;
            }
            c.eligible = w.leaf(CoverLeaf::Eligible, c) != 0;
            c.saved_reservation = c.reservation.clone();
            if !c.eligible {
                w.class(4);
                return;
            }
            c.posture = w.leaf(CoverLeaf::Posture(0, false), c) as i32;
            if !w.plays(id(c, 0)) {
                w.play(id(c, 0), f32::from_bits(0x3EAA7EFA), 1.0);
            }
            c.destination = c
                .reservation
                .as_ref()
                .expect("eligible cover requires node")
                .position;
            c.flags |= 0x20000;
            enter(c, 1);
            w.leaf(CoverLeaf::Face, c);
        }
        1 => {
            let pos = c
                .reservation
                .as_ref()
                .expect("cover state requires node")
                .position;
            let delta = std::array::from_fn::<_, 3, _>(|n| c.position[n] - pos[n]);
            let d = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
            if d > 0.1 {
                w.leaf(CoverLeaf::Align(0.05), c);
            } else {
                c.previous = 1;
                c.state = 2;
                c.pending = 2;
                c.state_time = 0.0;
            }
            w.leaf(CoverLeaf::Face, c);
        }
        2 => {
            if w.finished() {
                if !c.selected {
                    w.leaf(CoverLeaf::Assess(true), c);
                }
                let decision = w.leaf(CoverLeaf::Decision, c);
                if (decision as i32) < 8 {
                    if decision > 9 {
                        w.class(4);
                        return;
                    }
                    if decision != 0 {
                        c.posture = w
                            .leaf(CoverLeaf::Posture(decision, (4..=6).contains(&decision)), c)
                            as i32;
                    }
                    if c.posture == 0 {
                        w.class(4);
                        return;
                    }
                } else {
                    let tag = c
                        .reservation
                        .as_ref()
                        .expect("cover state requires node")
                        .tag;
                    let n = w.resolve_node(tag);
                    c.node = if n < 0 { c.player_node } else { n };
                    c.walk_timer = 0;
                    c.walk_progress = 0;
                    w.leaf(CoverLeaf::MarkNode(n), c);
                    c.node_flags |= 0x40000000;
                }
                c.clock = (unit(w.rng_raw()) * 0.5) as f32;
                match decision {
                    0 => enter(c, 3),
                    1 | 2 | 5 | 6 => {
                        c.action_deadline = 0.0;
                        c.unseen = 0.0;
                        let (table, state) = match decision {
                            1 => (2, 4),
                            2 => (6, 5),
                            5 => (10, 8),
                            _ => (14, 9),
                        };
                        w.play(id(c, table), 0.0, 1.0);
                        enter(c, state);
                    }
                    3 | 4 | 7 => {
                        let (table, state) = match decision {
                            3 => (1, 6),
                            4 => (8, 7),
                            _ => (16, 10),
                        };
                        w.play(id(c, table), 0.0, 1.0);
                        enter(c, state);
                    }
                    8 | 9 => {
                        let clip = w.leaf(CoverLeaf::TurnClip, c);
                        w.play(clip, 0.0, 1.0);
                        if c.posture < 10 && c.posture > -10 {
                            c.posture = 0;
                        }
                        enter(c, 11);
                    }
                    _ => {
                        w.class(4);
                        return;
                    }
                }
            }
            common(c, w, Some(0.05), true);
        }
        3 => {
            if c.clock >= 1.0 {
                c.previous = 3;
                c.state = 2;
                c.state_time = 0.0;
                c.pending = 2;
            }
            if w.finished() && !w.plays(id(c, 9)) && !w.plays(id(c, 0)) {
                let which = if !w.plays(0x592) { 0 } else { 9 };
                w.play(id(c, which), f32::from_bits(0x3EAA7EFA), 1.0);
            }
            common(c, w, Some(0.01), true);
        }
        4 | 8 => sustain(c, w, aware, c.state == 8),
        5 | 9 => {
            let (entry, idle) = if c.state == 5 { (6, 0) } else { (14, 9) };
            if !w.plays(id(c, entry)) && !w.plays(id(c, idle)) {
                w.play(id(c, idle), f32::from_bits(0x3EAA7EFA), 1.0);
            }
            if w.transition(id(c, entry)) || w.transition(id(c, idle)) {
                w.play(id(c, idle), 0.0, 1.0);
                enter(c, 2);
            } else {
                if !w.plays(id(c, idle)) {
                    if w.plays(id(c, entry)) {
                        if !c.selected {
                            w.leaf(CoverLeaf::Assess(false), c);
                        }
                        if aware {
                            c.unseen = 0.0;
                        }
                        if w.event_ready(0x200) {
                            w.leaf(CoverLeaf::Fire, c);
                        }
                    }
                } else {
                    w.leaf(CoverLeaf::Align(0.01), c);
                }
                w.leaf(CoverLeaf::Face, c);
            }
        }
        6 => {
            if w.finished() {
                enter(c, 2);
            }
            common(c, w, None, true);
        }
        7 | 10 => {
            let (idle, entry) = if c.state == 7 { (9, 8) } else { (0, 16) };
            common(c, w, Some(0.01), true);
            if w.finished() && !w.plays(id(c, idle)) {
                let b = if !w.plays(id(c, entry)) {
                    f32::from_bits(0x3EAA7EFA)
                } else {
                    0.0
                };
                w.play(id(c, idle), b, 1.0);
                return;
            }
            if c.clock > 3.0 {
                enter(c, 2);
            }
        }
        11 => {
            let p = c
                .reservation
                .as_ref()
                .expect("cover state requires node")
                .position;
            let d = std::array::from_fn::<_, 3, _>(|n| c.position[n] - p[n]);
            let distance = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            let angle = if !c.selected { w.target_angle(c) } else { 0.0 };
            if w.finished() || distance <= 0.8 {
                match c.posture {
                    -90 => {
                        let b = if w.plays(0x5da) {
                            0.0
                        } else {
                            f32::from_bits(0x3EAA7EFA)
                        };
                        w.play(0x5f2, b, 1.0);
                    }
                    90 => {
                        let b = if w.plays(0x5ee) {
                            0.0
                        } else {
                            f32::from_bits(0x3EAA7EFA)
                        };
                        w.play(0x5f1, b, 1.0);
                    }
                    _ => {
                        w.leaf(CoverLeaf::Face, c);
                        if !w.plays(0x5f0) {
                            w.play(0x5f0, f32::from_bits(0x3EAA7EFA), 1.0);
                        }
                    }
                }
                let tag = c
                    .saved_reservation
                    .as_ref()
                    .expect("exit cover requires saved node")
                    .tag;
                w.leaf(CoverLeaf::Unreserve(tag), c);
                c.node_flags &= !0x40000000;
                w.class(0x10000);
            }
            if c.state_time <= 0.5 || angle.abs() >= 70.0 {
                c.flags &= !0x60000;
                w.leaf(CoverLeaf::Blend(1), c);
                w.leaf(CoverLeaf::Walk, c);
            } else {
                let old = c.flags;
                c.flags = old | 2;
                if c.clip_policy & 0x100 != 0 {
                    c.flags = (old & !0x40000) | 2;
                    if c.slot_flags & 1 != 0 {
                        w.leaf(CoverLeaf::Blend(2), c);
                        w.leaf(CoverLeaf::Walk, c);
                        return;
                    }
                }
                w.leaf(CoverLeaf::Walk, c);
            }
        }
        12 => {
            if c.action_timer > 0.3 {
                w.leaf(CoverLeaf::ExitMotion, c);
            }
            if c.state_time > 0.3 {
                if let Some(node) = &c.saved_reservation {
                    w.leaf(CoverLeaf::Unreserve(node.tag), c);
                    c.node_flags &= !0x40000000;
                }
                c.reservation = None;
                c.flags &= !0x60000;
                w.leaf(CoverLeaf::Blend(1), c);
                w.class(4);
            }
            w.leaf(CoverLeaf::Walk, c);
        }
        _ => {}
    }
}
