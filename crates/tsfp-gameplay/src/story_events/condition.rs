//! Condition kinds (spec 4.4, `0x2359A0`, jump table `0x235F1C`, kinds 0 to 37). Kinds 2, 28
//! and 32 read the slot manager. Kinds 1, 3 to 9, 29 to 31, 33, 36 and 37 read world state that
//! only the game world has, so they go through [`World::condition_query`] and stay Unsupported
//! (never a guess) when the world does not answer. Kinds above 37 are false in the original.
//!
//! Decoded scene predicates (MEASURED instruction reading, game-specific, names INFERRED):
//! * 1 (`0x1DE560`): the object with tag 0x44524157 has extension `+0xAC` equal to 0.
//! * 2 (`0x1E4630`): slot 5 is finished. 28 (`0x1ECA50`): slot 2 is finished. Both native here.
//! * 3 (`0x1E1FD0`): the first player's record `+0x678` is not -1 and the player state word
//!   `0x1568` of the first player selects (jump table `0x1E2008`) between true and false.
//! * 4 (`0x1DFF20`): tag 0x574C4452 not found by `0xB2350`/`0xB0F60`, and tag 0x574E4C46 within
//!   a distance of every (or the nearest) player when at least four players exist.
//! * 5, 6 (`0x1EC980`, `0x1EC9C0`): actor tagged 0x47415330 / 0x47415331 has `+0x34 == 0x4C` and
//!   the scene word `[0x75BF0C]` is 0. 7, 8 (`0x1EC900`, `0x1EC940`): same actors with `[0x75BF0C] == 1`
//!   and health `+0x1CC` above 0. 9 (`0x1ECA00`): either of the two actors has `+0x34 == 0x4C`.
//! * 29 (`0x1EC8E0`): object tagged 0x47415330 exists, or `[0x75BF0C] == 1`.
//! * 30 (`0x1ED630`): object tagged 0x41525452 exists and its extension float `+0x10` is within
//!   a small band of 10.0 and `+0xA8` within a band of 0.
//! * 31 (`0x1E04F0`): scene float `[0x75BC04]+0x768` against a constant, plus a particle spawn side effect.

use super::engine::EngineError;
use super::record::Condition;
use super::slots::SlotManager;
use super::world::{ConditionQuery, IdVariant, World};

const UNSET: u32 = u32::MAX;
/// Slot ids hard-coded in kinds 2 and 28.
const SLOT_KIND_2: u32 = 5;
const SLOT_KIND_28: u32 = 2;

pub fn is_supported(kind: u32) -> bool {
    matches!(kind, 0 | 2 | 10..=28 | 32 | 34 | 35)
}

/// Kinds that need a world answer ([`World::condition_query`]).
pub fn is_world_query(kind: u32) -> bool {
    matches!(kind, 1 | 3..=9 | 29..=31 | 33 | 36 | 37)
}

pub fn evaluate(
    condition: &Condition,
    world: &mut dyn World,
    slots: &SlotManager,
) -> Result<bool, EngineError> {
    use IdVariant::{CharacterId as A, FollowingLink as B, ResourceMembership as C};
    let ids: Vec<u32> = condition
        .args
        .iter()
        .copied()
        .filter(|&id| id != UNSET)
        .collect();
    let count = world.player_count();
    let any =
        |world: &dyn World, variant| (0..count).any(|p| world.player_matches(p, variant, &ids));
    let all =
        |world: &dyn World, variant| (0..count).all(|p| world.player_matches(p, variant, &ids));
    // Kinds 23 to 27 test the player returned by `0x277FE0`, which can be null.
    let first = |world: &dyn World, variant| {
        world.first_player_available() && world.player_matches(0, variant, &ids)
    };
    let query = |world: &mut dyn World, query: ConditionQuery| {
        world
            .condition_query(&query)
            .ok_or(EngineError::Unsupported {
                what: "condition kind",
                value: condition.kind,
            })
    };
    Ok(match condition.kind {
        0 => true,
        10 => false,
        11 => any(world, A),
        12 => all(world, A),
        13 => !any(world, A),
        14 => any(world, B),
        15 => any(world, C),
        16 => count > 1 && world.player_matches(1, B, &ids),
        17 => all(world, B),
        18 => all(world, C),
        19 => !all(world, C),
        20 => !all(world, B),
        21 => !any(world, B),
        22 => !any(world, C),
        23 => first(world, A),
        24 => first(world, B),
        25 => !first(world, B),
        26 => first(world, C),
        27 => world.first_player_available() && world.first_player_flag(),
        2 => slots.is_finished(SLOT_KIND_2),
        28 => slots.is_finished(SLOT_KIND_28),
        32 => slots.is_finished(condition.args[0]),
        34 => !world.tag_exists(condition.args[0]),
        35 => world.tag_exists(condition.args[0]),
        1 | 3..=9 | 29..=31 => query(
            world,
            ConditionQuery::Scene {
                kind: condition.kind,
                args: condition.args,
            },
        )?,
        33 => query(
            world,
            ConditionQuery::TableField {
                key: condition.args[0],
            },
        )?,
        36 => query(
            world,
            ConditionQuery::ChildState {
                key: condition.args[0],
            },
        )?,
        37 => query(world, ConditionQuery::FewerThanTwoOfClass)?,
        // `ja 0x235f14` in the original: out of range kinds are false.
        _ => false,
    })
}
