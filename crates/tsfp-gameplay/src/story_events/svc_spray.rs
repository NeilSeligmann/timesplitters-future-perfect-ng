//! T1870: the spray services of the effect entries, `0x345B60` (burst emitter), `0x345B10` (owner mask), `0xD2D20` (player slot of an
//! object) and `0x345C60` (streak sprite with the sound pick `0x345710`), ported from the original.
//!
//! Source `build/default.xbe` (SHA-256 `3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc`), validated against the
//! original under the Unicorn oracle (`tools/t1846_gold/svc_spray.py`, test `tests/story_events_t1870_spray.rs`). Document
//! `docs/t-story-event-pool-classes-t1870.md`. Behaviour MEASURED (xemu level), names INFERRED. They are called from `0xEFF90`
//! (kinds `0x2C..=0x2E`, [`super::svc_effect_tick`]) and by nine other callbacks of the class table (`0xE8B47`, `0xEA070`, `0xF03B4`, ...).
//!
//! * `0x345B60(mode, owner, handle, &position, &vector)`: picks the template (`0` `0x766AD0`, `1` `0x766CB0`, `2` `0x766C28`, anything else
//!   returns), creates the emitter (`0x111AE0(handle, template, 0, &position, &vector)`) and then runs the owner mask `0x345B10(owner)`.
//! * `0x345B10(owner)`: for an owner whose `+0x20` is 8, with a player record at `+0x7C` whose `+8` is below the player count
//!   `[0x790950]` (signed), the slot of the player's entity (`0xD2D20([[record] + 0x150])`) is turned into the mask `!(1 << slot)` and
//!   handed to `0x10E680` (it sets the visibility words `+0x78`/`+0x7C` of the current emitter `[0x7AD28C]`); a negative slot does nothing.
//! * `0xD2D20(object)`: `[object + 8]` is an entity pointer (0 gives -1); the slot is `(entity - [0x7B0CE4]) / 0xFFC` by the compiler's
//!   multiply and shift sequence (reproduced exactly).
//! * `0x345C60(&position, &vector, cell, scale, kind)`: one rng draw gives `((u * 2^-32) * 0.9 + 0.3) * scale` (x87 double, stored as f32),
//!   a second draw picks the sprite id by `rng % 5` (`0x3C2 0x3BE 0x3BF 0x3C1 0x3C3`, looked up through `0x80F20`), and the streak is
//!   created by `0x1128C0(id, 1, 0x7F323232, kind, f, 1, cell, &position, &vector)` (an engine service: ray cast `0x88C60` and a sprite
//!   record, behind [`SprayHost::streak_sprite`]).

/// The three burst templates of modes 0, 1 and 2.
pub const BURST_TEMPLATES: [u32; 3] = [0x0076_6AD0, 0x0076_6CB0, 0x0076_6C28];
/// Player count `[0x790950]`, entity base `[0x7B0CE4]`, current emitter `[0x7AD28C]`.
pub const PLAYER_COUNT: u32 = 0x0079_0950;
pub const ENTITY_BASE: u32 = 0x007B_0CE4;
/// Sprite ids of `rng % 5` for the streak (0 to 3, other).
pub const STREAK_SPRITES: [u32; 5] = [0x3C2, 0x3BE, 0x3BF, 0x3C1, 0x3C3];
/// Colour argument of the streak sprite.
pub const STREAK_COLOR: u32 = 0x7F32_3232;

/// Arguments of `0x1128C0` in call order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreakSprite {
    pub sprite: u32,
    pub color: u32,
    pub kind: i32,
    pub length: f32,
    pub cell: u32,
    pub position: [f32; 3],
    pub vector: [f32; 3],
}

/// Engine side of the spray services.
pub trait SprayHost {
    /// `0x153B40`: one rng draw.
    fn rng(&mut self) -> u32;
    /// A word of guest memory (read only).
    fn word(&mut self, address: u32) -> u32;
    /// `0x111AE0(handle, template, flags, &position, &vector)`.
    fn create_emitter(
        &mut self,
        handle: u32,
        template: u32,
        flags: u32,
        position: [f32; 3],
        vector: [f32; 3],
    );
    /// `0x10E680(mask)`.
    fn emitter_mask(&mut self, mask: u32);
    /// `0x80F20(index)`: the table value of a sprite index.
    fn sprite_value(&mut self, index: u32) -> u32;
    /// `0x1128C0(...)`.
    fn streak_sprite(&mut self, call: StreakSprite);
}

/// `0xD2D20([object])` for an entity word: `(entity - base) / 0xFFC` by the original's sequence, -1 for a null entity.
pub fn player_slot(entity: u32, base: u32) -> i32 {
    if entity == 0 {
        return -1;
    }
    let difference = entity.wrapping_sub(base) as i32;
    let product = i64::from(difference) * i64::from(0x8020_0803u32 as i32);
    let high = ((product >> 32) as i32).wrapping_add(difference);
    let quotient = high >> 11;
    quotient.wrapping_add(((quotient as u32) >> 31) as i32)
}

/// `0x345B10(owner)`.
pub fn owner_mask(host: &mut dyn SprayHost, owner: u32) {
    if owner == 0 || host.word(owner.wrapping_add(0x20)) != 8 {
        return;
    }
    let record = host.word(owner.wrapping_add(0x7C));
    if record == 0 {
        return;
    }
    if (host.word(record.wrapping_add(8)) as i32) >= host.word(PLAYER_COUNT) as i32 {
        return;
    }
    let player = host.word(record);
    let object = host.word(player.wrapping_add(0x150));
    let entity = host.word(object.wrapping_add(8));
    let slot = player_slot(entity, host.word(ENTITY_BASE));
    if slot < 0 {
        return;
    }
    host.emitter_mask(!(1u32.wrapping_shl(slot as u32)));
}

/// `0x345B60(mode, owner, handle, &position, &vector)`.
pub fn burst(
    host: &mut dyn SprayHost,
    mode: u32,
    owner: u32,
    handle: u32,
    position: [f32; 3],
    vector: [f32; 3],
) {
    let Some(&template) = BURST_TEMPLATES.get(mode as usize) else {
        return;
    };
    host.create_emitter(handle, template, 0, position, vector);
    owner_mask(host, owner);
}

/// `0x345710`: the sprite value of `rng % 5`.
fn pick_sprite(host: &mut dyn SprayHost) -> u32 {
    let index = host.rng() % 5;
    host.sprite_value(STREAK_SPRITES[index as usize])
}

/// `0x345C60(&position, &vector, cell, scale, kind)`.
pub fn streak(
    host: &mut dyn SprayHost,
    position: [f32; 3],
    vector: [f32; 3],
    cell: u32,
    scale: f32,
    kind: i32,
) {
    let unit = f64::from(host.rng()) * f64::from(f32::from_bits(0x2F80_0000));
    let length = ((unit * f64::from(f32::from_bits(0x3F66_6666))
        + f64::from(f32::from_bits(0x3E99_999A)))
        * f64::from(scale)) as f32;
    let sprite = pick_sprite(host);
    host.streak_sprite(StreakSprite {
        sprite,
        color: STREAK_COLOR,
        kind,
        length,
        cell,
        position,
        vector,
    });
}
