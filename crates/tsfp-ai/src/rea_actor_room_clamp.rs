//! Readable typed port of the `0x18ACC0` room-bound operation adjacent to the
//! T2435 AI frontier. The original role names are inferred from its behavior.
//!
//! Verification: Tier 0 (native typed implementation with unit smoke tests).
//! It is not wired into the live actor runtime or enabled by an adoption mask.

/// One of the two room bounds returned by the original `0x873F0` service.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// The six floats accessed by `0x18ACC0` in its second input record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActorKinematics {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
}

fn distance_outside(position: f32, min: f32, max: f32) -> f32 {
    if min > position {
        min - position
    } else if position > max {
        position - max
    } else {
        0.0
    }
}

fn room_distance(position: [f32; 3], bounds: RoomBounds) -> f32 {
    let x = distance_outside(position[0], bounds.min[0], bounds.max[0]);
    let y = distance_outside(position[1], bounds.min[1], bounds.max[1]);
    let z = distance_outside(position[2], bounds.min[2], bounds.max[2]);
    (x + y) + z
}

fn clamp_axis(position: &mut f32, velocity: &mut f32, min: f32, max: f32) {
    if min > *position {
        *position = min;
        *velocity = velocity.abs();
    } else if *position > max {
        *position = max;
        *velocity = -velocity.abs();
    }
}

/// Select the room with the smallest summed distance outside its AABB and
/// clamp position into it. Equal scores preserve the first room. When the
/// current room key already matches either supplied room key, the original
/// returns without mutating kinematics.
///
/// `room_keys` and `bounds` are supplied by the caller in the order of the
/// original two entries from `[0x4E8CE4]` and `[0x4E8CE8]`; the typed function
/// does not read guest memory or own the room table.
pub fn clamp_to_nearest_room(
    current_room_key: u32,
    room_keys: [u32; 2],
    bounds: [RoomBounds; 2],
    actor: &mut ActorKinematics,
) {
    if room_keys.contains(&current_room_key) {
        return;
    }

    let mut chosen = 0;
    let mut best_distance = f32::MAX;
    for index in 0..2 {
        let distance = room_distance(actor.position, bounds[index]);
        if distance < best_distance {
            chosen = index;
            best_distance = distance;
        }
    }

    for axis in 0..3 {
        clamp_axis(
            &mut actor.position[axis],
            &mut actor.velocity[axis],
            bounds[chosen].min[axis],
            bounds[chosen].max[axis],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{ActorKinematics, RoomBounds, clamp_to_nearest_room};

    const LEFT: RoomBounds = RoomBounds {
        min: [-10.0, -2.0, -2.0],
        max: [0.0, 2.0, 2.0],
    };
    const RIGHT: RoomBounds = RoomBounds {
        min: [4.0, -2.0, -2.0],
        max: [14.0, 2.0, 2.0],
    };

    #[test]
    fn room_key_guard_preserves_state() {
        let mut actor = ActorKinematics {
            position: [30.0, 3.0, -4.0],
            velocity: [-1.0, 2.0, 3.0],
        };
        let before = actor;
        clamp_to_nearest_room(20, [10, 20], [LEFT, RIGHT], &mut actor);
        assert_eq!(actor, before);
    }

    #[test]
    fn nearest_room_clamps_position_and_reflects_outward_velocity() {
        let mut actor = ActorKinematics {
            position: [15.0, 3.0, -3.0],
            velocity: [2.0, -4.0, 5.0],
        };
        clamp_to_nearest_room(99, [10, 20], [LEFT, RIGHT], &mut actor);
        assert_eq!(actor.position, [14.0, 2.0, -2.0]);
        assert_eq!(actor.velocity, [-2.0, -4.0, 5.0]);
    }

    #[test]
    fn equal_room_scores_keep_the_first_room() {
        let mut actor = ActorKinematics {
            position: [2.0, 0.0, 0.0],
            velocity: [0.0; 3],
        };
        clamp_to_nearest_room(99, [10, 20], [LEFT, RIGHT], &mut actor);
        assert_eq!(actor.position[0], 0.0);
    }
}
