//! Complete six-state combat tick 0x2B7F30, with engine leaves explicit.
#[derive(Clone, Debug)]
pub struct CombatPeer {
    pub object: u32,
    pub position: [f32; 3],
    pub excluded: bool,
    pub team: u32,
}
#[derive(Clone, Debug)]
pub struct CombatCtx {
    pub state: u32,
    pub pending: i32,
    pub previous: u32,
    pub state_time: f32,
    pub auxiliary_flags: u8,
    pub drain: f32,
    pub tick_rate: i32,
    pub delta_ticks: f32,
    pub flags: u32,
    pub actor: u32,
    pub selected: u32,
    pub player_count: u32,
    pub model: u32,
    pub category: u8,
    pub object_present: bool,
    pub object_flags: u32,
    pub object_type: u16,
    pub target: u32,
    pub target_excluded: bool,
    pub target_is_mode_target: bool,
    pub weapon_a: u32,
    pub weapon_b: u32,
    pub weapon_cooldown: u32,
    pub random_threshold: f32,
    pub blocked_count: i32,
    pub origin: i32,
    pub destination_node: i32,
    pub target_node: i32,
    pub player_node: i32,
    pub walk_timer: u32,
    pub walk_progress: u32,
    pub position: [f32; 3],
    pub target_position: [f32; 3],
    pub destination: [f32; 3],
    pub target_distance_squared: f32,
    pub clock: f32,
    pub unseen: f32,
    pub action_deadline: f32,
    pub action_word: u32,
    pub room_blocked: bool,
    pub target_room_blocked: bool,
    pub mode: u8,
    pub submode: u8,
    pub team: u32,
    pub peers: Vec<CombatPeer>,
}
#[derive(Clone, Copy, Debug)]
pub enum CombatLeaf {
    Navigate,
    Weapon,
    Prepare,
    Blocked,
    Follow,
    RandomThreshold,
    NoWeapon,
    Armed,
    Selected,
    Path,
    LateSelected,
    Fire,
}
impl CombatLeaf {
    pub fn original_address(self) -> u32 {
        match self {
            Self::Navigate => 0x15d470,
            Self::Weapon => 0x15b1b0,
            Self::Prepare => 0x27d510,
            Self::Blocked => 0x124f30,
            Self::Follow => 0x27c9a0,
            Self::RandomThreshold => 0x15a2d0,
            Self::NoWeapon => 0x267710,
            Self::Armed => 0x2b0b10,
            Self::Selected => 0x2b53f0,
            Self::Path => 0x15d500,
            Self::LateSelected => 0x2b4870,
            Self::Fire => 0x161d40,
        }
    }
}
#[derive(Clone, Debug)]
pub struct CombatDamage {
    pub object: u32,
    pub amount: f32,
    pub radial: bool,
    pub direction: [f32; 3],
    pub distance: f32,
}
pub trait CombatWorld {
    fn awareness(&mut self, c: &mut CombatCtx) -> bool;
    /// All original engine helpers may update the modeled context. RandomThreshold
    /// is represented by random_threshold(), since it returns an x87 float.
    fn leaf(&mut self, l: CombatLeaf, c: &mut CombatCtx) -> u32;
    fn random_threshold(&mut self) -> f32;
    fn ready(&mut self, blend: f32) -> bool;
    fn plays(&mut self, id: u32) -> bool;
    fn transition(&mut self, id: u32) -> bool;
    fn finished(&mut self) -> bool;
    fn event(&mut self, bits: u32) -> bool;
    fn policy(&mut self, id: u32) -> u32;
    fn selector(&mut self, id: u32) -> u32;
    fn rng_raw(&mut self) -> u32;
    fn play(&mut self, id: u32, blend: f32, speed: f32);
    fn class(&mut self, mask: u64);
    fn damage_init(&mut self);
    fn double_damage(&mut self, c: &CombatCtx) -> bool;
    fn in_cone(&mut self, position: [f32; 3]) -> bool;
    fn clamp(&mut self, value: f32) -> f32;
    fn damage(&mut self, request: CombatDamage);
}
fn enter(c: &mut CombatCtx, s: u32) {
    if c.state != s {
        c.previous = c.state;
        c.state = s;
        c.pending = s as i32;
        c.state_time = 0.0;
    }
}
fn inactive(c: &CombatCtx) -> bool {
    c.object_present
        && c.actor >= c.player_count
        && (c.object_flags & 0x100 != 0 || c.object_type == 0)
}
fn deadline(c: &mut CombatCtx, w: &mut impl CombatWorld) {
    c.action_deadline = (f64::from(w.rng_raw()) / 4294967296.0 + 2.0 + f64::from(c.clock)) as f32;
}
// The four pools occupy 0x520D68+category*300, each count then up to14 ids.
#[path = "char_combat_tables.rs"]
mod tables;
fn pick(c: &CombatCtx, w: &mut impl CombatWorld, pool: usize) -> u32 {
    let pool = tables::POOLS[c.category as usize][pool];
    pool[w.rng_raw() as usize % pool.len()]
}
pub fn tick(c: &mut CombatCtx, w: &mut impl CombatWorld) {
    let aware = w.awareness(c);
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
    w.leaf(CombatLeaf::Navigate, c);
    w.leaf(CombatLeaf::Weapon, c);
    w.leaf(CombatLeaf::Prepare, c);
    if c.flags & 0x2000 != 0 || (w.leaf(CombatLeaf::Blocked, c) != 0 && c.blocked_count > 4) {
        if c.target == 0 {
            w.class(0x1000);
            return;
        }
        let dx = c.position[0] - c.target_position[0];
        let dz = c.position[2] - c.target_position[2];
        if dx * dx + dz * dz >= 4.0 {
            if c.origin != c.destination_node && w.leaf(CombatLeaf::Path, c) == 0 {
                c.destination_node = if c.target_node < 0 {
                    c.player_node
                } else {
                    c.target_node
                };
                c.walk_timer = 0;
                c.walk_progress = 0;
            }
            if !w.plays(0x26) {
                w.play(0x26, 0.333, 1.5);
            }
            return;
        }
        w.leaf(CombatLeaf::Follow, c);
        return;
    }
    let armed = c.weapon_a != 0 || c.weapon_b != 0;
    if c.model == 0x14 {
        if c.weapon_cooldown == 0
            && w.random_threshold() < c.random_threshold
            && (c.object_type == 0 || c.target_distance_squared <= 81.0)
        {
            c.flags |= 2;
        }
    } else if aware && armed && c.weapon_cooldown == 0 && w.random_threshold() < c.random_threshold
    {
        c.flags |= 2;
    }
    let special = if !armed {
        if w.leaf(CombatLeaf::NoWeapon, c) != 0 {
            return;
        }
        true
    } else {
        w.leaf(CombatLeaf::Armed, c) != 0
    };
    if c.state != 5 && inactive(c) && c.model != 0x36 {
        enter(c, 5);
    }
    if c.actor == c.selected {
        w.leaf(CombatLeaf::Selected, c);
    }
    let range: f32 = if c.model == 0x36 {
        1.5
    } else if special {
        20.0
    } else {
        10.0
    };
    c.destination = if inactive(c) {
        c.position
    } else {
        c.target_position
    };
    match c.state {
        0 => {
            if w.ready(0.333) || w.transition(0x25) {
                if !special && c.model == 0x36 {
                    w.play(
                        if c.target_distance_squared <= range * range {
                            0x1e1
                        } else {
                            0x1e0
                        },
                        0.33,
                        1.2,
                    );
                    enter(c, 1);
                } else {
                    let id = if special { 0x1e3 } else { pick(c, w, 0) };
                    w.play(id, 0.33, 1.2);
                    enter(c, 3);
                }
            }
            c.clock = 0.0;
            c.action_deadline = 0.0;
        }
        1 => {
            if w.finished() {
                if w.plays(0x1e1) || w.plays(0x111) {
                    w.damage_init();
                    let amount = if w.double_damage(c) { 1.0 } else { 0.5 };
                    for p in c.peers.clone() {
                        let d = std::array::from_fn::<_, 3, _>(|i| p.position[i] - c.position[i]);
                        if d.iter().map(|v| v * v).sum::<f32>() <= 2.25 && w.in_cone(p.position) {
                            w.damage(CombatDamage {
                                object: p.object,
                                amount,
                                radial: false,
                                direction: [0.0; 3],
                                distance: 0.0,
                            });
                        }
                    }
                }
                let id = if c.target_distance_squared >= range * range {
                    if c.target_distance_squared >= (range + 2.0) * (range + 2.0) {
                        0x1e0
                    } else {
                        0x111
                    }
                } else {
                    0x1e1
                };
                w.play(id, 0.0, 1.2);
            }
        }
        2 | 4 => {
            if w.ready(0.333) {
                let id = if special {
                    if c.state == 2 { 0x1e4 } else { 0x1e5 }
                } else {
                    pick(c, w, if c.state == 2 { 2 } else { 3 })
                };
                if !w.plays(id) || w.policy(id) & 1 == 0 {
                    w.play(id, 0.33, 1.2);
                }
                if (c.state == 2 && c.target_distance_squared < range * range)
                    || (c.state == 4 && c.target_distance_squared > range * range)
                {
                    enter(c, 3);
                }
            }
        }
        3 => {
            if w.ready(0.333) {
                let mut id = if special { 0x1e3 } else { pick(c, w, 1) };
                if !special && w.selector(id) & 2 != 0 {
                    id = if w.plays(0x1c8) { 0x1cb } else { 0x1c8 };
                }
                w.play(id, 0.33, 1.2);
                if c.target_distance_squared > (range + 5.0) * (range + 5.0) {
                    enter(c, 2);
                } else if c.target_distance_squared < (range - 5.0) * (range - 5.0) {
                    enter(c, 4);
                }
            }
        }
        5 => {
            if !inactive(c) {
                enter(c, 3);
            }
            if !w.plays(0x25) {
                w.play(0x25, 0.33, 1.2);
            }
            if special && c.target != 0 && c.clock > c.action_deadline {
                if !c.room_blocked && !c.target_room_blocked {
                    w.damage_init();
                    for p in c.peers.clone() {
                        if p.excluded {
                            continue;
                        }
                        let d = std::array::from_fn::<_, 3, _>(|i| {
                            p.position[i] - c.target_position[i]
                        });
                        let distance = d.iter().map(|v| v * v).sum::<f32>().sqrt();
                        let mut amount = w.clamp(
                            (1.0 - f64::from(distance)
                                * f64::from(0.2_f32)
                                * f64::from(distance)
                                * f64::from(0.2_f32)) as f32,
                        ) * 6.0;
                        if amount != 0.0 {
                            if p.team == c.team {
                                amount *= 0.5;
                            }
                            w.damage(CombatDamage {
                                object: p.object,
                                amount,
                                radial: true,
                                direction: d,
                                distance,
                            });
                        }
                    }
                }
                deadline(c, w);
            }
        }
        _ => {}
    }
    if c.actor == c.selected && c.clock > 2.0 {
        w.leaf(CombatLeaf::LateSelected, c);
    }
    if (w.transition(0x1e5) || w.transition(0x1e3) || w.transition(0x1e4))
        && w.event(0x200)
        && c.state != 5
    {
        w.leaf(CombatLeaf::Fire, c);
        deadline(c, w);
    }
    if c.target == 0 || c.target_excluded {
        if matches!(c.mode, 2 | 3 | 7) {
            w.class(0x10);
            return;
        }
        if c.mode == 9 && (f64::from(w.rng_raw()) / 4294967296.0) < 0.2 && armed {
            c.action_word = 0;
            w.class(0x40000);
            return;
        }
        if c.submode != 0x12 || !c.target_is_mode_target {
            w.class(0x10);
            return;
        }
    }
    if aware {
        c.unseen = 0.0;
    } else if c.unseen > 2.0 {
        w.class(0x10);
    }
}
