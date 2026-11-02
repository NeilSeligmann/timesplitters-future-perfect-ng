//! Layer and body accessors of the animation object (T1668, batch 1 of the breadth-first port): `0x59CF0`, `0x59D50`, `0x59D70`, `0x59D90`,
//! `0x59DD0`, `0x59E00`, `0x59EC0`, `0x59F10`, `0x59F50`, `0x59F90`, `0x5A070`, `0x5A1E0`, `0x5A260`, `0x5A2B0`, `0x5A2F0`, `0x5BA00`, `0x61430`.
//!
//! Verification tier 1 (differential against the original running under Unicorn, 300 random worlds per function, `tools/t1668_layers_gold.py`,
//! `tests/layers_golden.rs`). The body memory is modelled as raw dwords ([`Body`]) so every field keeps its byte offset: an animation
//! object `obj` has `[obj+4]` = the body, `[obj+0x20]` its type, the body holds the layer states of [`crate::start`] at `+0xD4` (current)
//! and `+0x12C` (previous), the blend progress float at `+0x198` (> 0: the previous layer is the active one) and play speeds at `+0x108`
//! and `+0x160`.

/// Dwords of the body memory the accessors may touch (`0x1C0` bytes).
pub const BODY_DWORDS: usize = 0x1C0 / 4;
pub const CURRENT: usize = 0xD4;
pub const PREVIOUS: usize = 0x12C;
const BLEND: usize = 0x198;

/// Raw body memory, addressed by byte offset (dword aligned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body(pub [u32; BODY_DWORDS]);

impl Body {
    pub fn word(&self, offset: usize) -> u32 {
        self.0[offset / 4]
    }
    pub fn float(&self, offset: usize) -> f32 {
        f32::from_bits(self.word(offset))
    }
    fn set_float(&mut self, offset: usize, value: f32) {
        self.0[offset / 4] = value.to_bits();
    }
    /// Blend progress `+0x198`.
    pub fn blend(&self) -> f32 {
        self.float(BLEND)
    }
    /// `0x59CF0`: byte offset of the active layer: the previous one while blending, else the current one.
    pub fn active_layer(&self) -> usize {
        if self.blend() > 0.0 {
            PREVIOUS
        } else {
            CURRENT
        }
    }
    pub fn layer_id(&self, layer: usize) -> u32 {
        self.word(layer)
    }
    pub fn layer_slot(&self, layer: usize) -> usize {
        self.word(layer + 4) as usize
    }
    pub fn layer_time(&self, layer: usize) -> f32 {
        self.float(layer + 8)
    }
}

/// The clip world: `duration(slot)` is `[0x7E3954][slot] + 0x18` (`None` for an empty slot), `policy(slot)` the words at `0x7DE660 + 8 slot`.
pub trait ClipWorld {
    fn duration(&self, slot: usize) -> Option<f32>;
    fn policy(&self, slot: usize) -> (u32, u32);
}

/// `0x59D50`: duration of the current layer's clip (the original dereferences without a check, `None` here when empty).
pub fn current_duration(body: &Body, world: &impl ClipWorld) -> Option<f32> {
    world.duration(body.layer_slot(CURRENT))
}

/// `0x59D70` / `0x59D90`: does the current / previous layer play animation `id`.
pub fn current_plays(body: &Body, id: u32) -> bool {
    body.layer_id(CURRENT) == id
}
pub fn previous_plays(body: &Body, id: u32) -> bool {
    body.layer_id(PREVIOUS) == id
}

/// `0x59DD0(body)`: `body+0x17C` when the previous layer has an id, else `body+0x124` (0 for a null body).
pub fn secondary_word(body: Option<&Body>) -> u32 {
    match body {
        None => 0,
        Some(b) if b.word(PREVIOUS) == 0 => b.word(0x124),
        Some(b) => b.word(0x17C),
    }
}

fn policy_hit(world: &impl ClipWorld, slot: usize, masks: (u32, u32)) -> bool {
    world.duration(slot).is_some() && {
        let (p0, p1) = world.policy(slot);
        (p0 & masks.0) | (p1 & masks.1) != 0
    }
}

/// `0x59F10`: policy words of the current layer's clip overlap the masks (and the clip exists).
pub fn current_policy_hit(body: &Body, world: &impl ClipWorld, masks: (u32, u32)) -> bool {
    policy_hit(world, body.layer_slot(CURRENT), masks)
}
/// `0x59F50`: same for the previous layer.
pub fn previous_policy_hit(body: &Body, world: &impl ClipWorld, masks: (u32, u32)) -> bool {
    policy_hit(world, body.layer_slot(PREVIOUS), masks)
}
/// `0x59E00`: current or previous.
pub fn any_policy_hit(body: &Body, world: &impl ClipWorld, masks: (u32, u32)) -> bool {
    current_policy_hit(body, world, masks) || previous_policy_hit(body, world, masks)
}
/// `0x59F90`: the active layer only: while blending the previous layer, with a zero progress the current one, otherwise (negative or NaN progress) nothing.
pub fn active_policy_hit(body: &Body, world: &impl ClipWorld, masks: (u32, u32)) -> bool {
    let blend = body.blend();
    if blend > 0.0 && previous_policy_hit(body, world, masks) {
        return true;
    }
    blend == 0.0 && current_policy_hit(body, world, masks)
}

/// `0x59EC0` (x87 result): play position of the active layer, `time / duration`, 0.0 when the layer has no animation.
pub fn normalised_position(body: &Body, world: &impl ClipWorld) -> Option<f64> {
    let layer = body.active_layer();
    if body.layer_id(layer) == 0 {
        return Some(0.0);
    }
    let duration = world.duration(body.layer_slot(layer))?;
    Some(f64::from(body.layer_time(layer)) / f64::from(duration))
}

/// `0x5A1E0`: is `id` what the active layer plays (previous while blending, current otherwise, never with NaN progress).
pub fn active_plays(body: &Body, id: u32) -> bool {
    let blend = body.blend();
    if blend > 0.0 {
        body.layer_id(PREVIOUS) == id
    } else if blend.is_nan() {
        false
    } else {
        body.layer_id(CURRENT) == id
    }
}

/// `0x5A2B0`: time of the active layer.
pub fn active_time(body: &Body) -> f32 {
    body.layer_time(body.active_layer())
}

/// Clamp rule of `0x5A260` / `0x5A2F0`: with a clip the time is limited to the duration from above, and below 0 it becomes 0 (NaN passes).
fn clamp_time(duration: Option<f32>, time: f32) -> f32 {
    match duration {
        Some(d) if time > d => d,
        _ if 0.0 > time => 0.0,
        _ => time,
    }
}

/// `0x5A260(layer, time)`: sets `layer+8` and `layer+0xC`.
pub fn set_layer_time(body: &mut Body, layer: usize, world: &impl ClipWorld, time: f32) {
    let value = clamp_time(world.duration(body.layer_slot(layer)), time);
    body.set_float(layer + 0xC, value);
    body.set_float(layer + 8, value);
}

/// `0x5A2F0(obj, time)`: the same on the active layer.
pub fn set_active_time(body: &mut Body, world: &impl ClipWorld, time: f32) {
    let layer = body.active_layer();
    set_layer_time(body, layer, world, time);
}

/// `0x5BA00(body)`: speed 0 and both times of the current layer (`+0xDC`, `+0xE0`) to the duration when negative, else 0.
pub fn reset_current(body: &mut Body, world: &impl ClipWorld) {
    body.set_float(0x108, 0.0);
    let start = match world.duration(body.layer_slot(CURRENT)) {
        Some(d) if 0.0 > d => d,
        _ => 0.0,
    };
    body.set_float(0xE0, start);
    body.set_float(0xDC, start);
}

/// What `0x5A070` asks the world.
pub trait SpeedWorld {
    /// `[0x7DE458] & 0x800` and `[0x7DE45C] & 0x2000000 == 0`.
    fn slow_motion_allowed(&self) -> bool;
    /// `0xC6140()` result: the type word at `+0x218` of the player object, `None` for a null result.
    fn player_type(&self) -> Option<u32>;
    /// `[0x7DE4A4]`.
    fn slow_motion_scale(&self) -> f32;
}

/// `0x5A070(body, speed)` (`model_type` = `[[body]+0x20]`, `None` when the body has no model): stores the play speed at `+0x108` and
/// `+0x160`, scaled in the slow motion mode unless the player type is `0x1B`, and never negative for type 8 models.
pub fn set_speed(body: &mut Body, model_type: Option<u32>, world: &impl SpeedWorld, speed: f32) {
    let mut value = speed;
    if world.slow_motion_allowed() && matches!(world.player_type(), Some(t) if t != 0x1B) {
        value = world.slow_motion_scale() * speed;
    }
    if model_type == Some(8) && 0.0 > value {
        value = 0.0;
    }
    body.set_float(0x108, value);
    body.set_float(0x160, value);
}

/// `0x61430(obj)`: 1 when `resident(id)` (`0x56FC0`) is nonzero for the id of the active layer.
pub fn active_clip_resident(body: &Body, resident: impl Fn(u32) -> bool) -> bool {
    resident(body.layer_id(body.active_layer()))
}

/// A simple map backed world for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MapWorld {
    pub clips: std::collections::BTreeMap<usize, (f32, (u32, u32))>,
}

#[cfg(test)]
impl ClipWorld for MapWorld {
    fn duration(&self, slot: usize) -> Option<f32> {
        self.clips.get(&slot).map(|c| c.0)
    }
    fn policy(&self, slot: usize) -> (u32, u32) {
        self.clips.get(&slot).map_or((0, 0), |c| c.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Body {
        let mut b = Body([0; BODY_DWORDS]);
        b.0[CURRENT / 4] = 11;
        b.0[CURRENT / 4 + 1] = 1;
        b.0[PREVIOUS / 4] = 12;
        b.0[PREVIOUS / 4 + 1] = 2;
        b
    }

    #[test]
    fn active_layer_follows_blend_progress() {
        let mut b = body();
        assert_eq!(b.active_layer(), CURRENT);
        b.set_float(BLEND, 0.5);
        assert_eq!(b.active_layer(), PREVIOUS);
        assert!(active_plays(&b, 12) && !active_plays(&b, 11));
        b.set_float(BLEND, f32::NAN);
        assert!(!active_plays(&b, 11));
    }

    #[test]
    fn time_is_clamped_to_the_clip() {
        let mut world = MapWorld::default();
        world.clips.insert(1, (2.0, (0, 0)));
        let mut b = body();
        set_active_time(&mut b, &world, 5.0);
        assert_eq!(b.layer_time(CURRENT), 2.0);
        set_active_time(&mut b, &world, -1.0);
        assert_eq!(b.layer_time(CURRENT), 0.0);
        assert!(!current_policy_hit(&b, &world, (1, 1)));
        world.clips.insert(1, (2.0, (4, 0)));
        assert!(current_policy_hit(&b, &world, (4, 0)));
    }
}
